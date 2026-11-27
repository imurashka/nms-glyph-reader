use std::collections::HashSet;
use std::ops::RangeInclusive;

use super::shapes::Layer;
use crate::picture::Rect;

pub const GLYPHS: usize = 12;

const MOST_EMPTY_PLACES: usize = 2;
const PIECE_HEIGHTS: RangeInclusive<i32> = 3..=200;
const WIDEST_PIECE: i32 = 300;
const GLYPH_HEIGHTS: RangeInclusive<i32> = 8..=160;
const NARROWEST_GLYPH: f64 = 0.15;
const WIDEST_GLYPH: f64 = 2.5;
const BAND_ABOVE_AND_BELOW: f64 = 0.7;
const TALLEST_IN_BAND: f64 = 1.4;
const SHORTEST_MARK: f64 = 0.35;
const WIDEST_MARK: f64 = 2.2;
const PITCHES: RangeInclusive<f64> = 0.8..=3.5;
const WALK_REACH: f64 = 0.3;
const LINE_REACH: f64 = 0.25;
const CELL_REACH: f64 = 0.35;
const EMPTY_CELL_HALF_WIDTH: f64 = 0.4;
const SHORTEST_IN_ROW: f64 = 0.25;
const FIRST_FIT_SPAN: i32 = 2;
const SECOND_FIT_SPAN: i32 = 5;
const FEWEST_TO_FIT: usize = 2;
const TYPICAL: (i32, i32) = (3, 4);

#[derive(Debug)]
pub struct Row {
    pub cells: [Rect; GLYPHS],
    pub whole: bool,
}

struct Band {
    pieces: Vec<Rect>,
    marks: Vec<Rect>,
    centres: Vec<f64>,
}

struct Place {
    x: f64,
    pitch: f64,
    mark: Option<usize>,
}

pub fn find(layers: &[Layer]) -> Vec<Vec<Row>> {
    layers
        .iter()
        .map(|layer| in_shapes(&layer.shapes, layer.mask.width as i32))
        .collect()
}

pub fn glyph_sized(shape: &Rect) -> bool {
    let (width, height) = (f64::from(shape.width()), f64::from(shape.height()));
    GLYPH_HEIGHTS.contains(&shape.height())
        && (NARROWEST_GLYPH * height..=WIDEST_GLYPH * height).contains(&width)
}

fn in_shapes(shapes: &[Rect], picture_width: i32) -> Vec<Row> {
    let pieces: Vec<Rect> = shapes
        .iter()
        .copied()
        .filter(|s| PIECE_HEIGHTS.contains(&s.height()) && s.width() <= WIDEST_PIECE)
        .collect();
    let mut by_top: Vec<usize> = (0..pieces.len()).collect();
    by_top.sort_by_key(|&i| pieces[i].top);
    let mut rows = Vec::new();
    let mut seen: HashSet<(i32, Vec<Rect>)> = HashSet::new();
    for seed in pieces.iter().filter(|s| glyph_sized(s)) {
        let band = band_around(seed, &pieces, &by_top);
        let new = band.marks.len() >= GLYPHS - MOST_EMPTY_PLACES
            && seen.insert((seed.height(), band.pieces.clone()));
        if !new {
            continue;
        }
        let height = f64::from(seed.height());
        for (i, pair) in band.centres.windows(2).enumerate() {
            let pitch = pair[1] - pair[0];
            if !(PITCHES.start() * height..=PITCHES.end() * height).contains(&pitch) {
                continue;
            }
            let walked = walked_places(&band.centres, i, pitch);
            rows.extend(rows_on(&walked, &band, picture_width));
            if let Some(lined) = lined_places(&band.centres, pair[0], pitch) {
                rows.extend(rows_on(&lined, &band, picture_width));
            }
        }
    }
    rows.sort_unstable_by_key(|row| row.cells);
    rows.dedup_by_key(|row| row.cells);
    rows
}

fn band_around(seed: &Rect, pieces: &[Rect], by_top: &[usize]) -> Band {
    let height = f64::from(seed.height());
    let middle = f64::from(seed.top + seed.bottom) / 2.0;
    let top = middle - BAND_ABOVE_AND_BELOW * height;
    let bottom = middle + BAND_ABOVE_AND_BELOW * height;
    let first = by_top.partition_point(|&i| f64::from(pieces[i].top) < top);
    let mut inside: Vec<usize> = by_top[first..]
        .iter()
        .copied()
        .take_while(|&i| f64::from(pieces[i].top) <= bottom)
        .filter(|&i| {
            let piece = &pieces[i];
            f64::from(piece.bottom) <= bottom
                && f64::from(piece.height()) <= TALLEST_IN_BAND * height
        })
        .collect();
    inside.sort_unstable_by_key(|&i| (pieces[i].left, i));
    let mut joined: Vec<Rect> = Vec::new();
    for piece in inside.iter().map(|&i| &pieces[i]) {
        match joined.last_mut() {
            Some(last) if piece.left <= last.right => *last = last.joined(piece),
            _ => joined.push(*piece),
        }
    }
    let marks: Vec<Rect> = joined
        .iter()
        .copied()
        .filter(|piece| {
            f64::from(piece.height()) >= SHORTEST_MARK * height
                && f64::from(piece.width()) <= WIDEST_MARK * height
        })
        .collect();
    Band {
        centres: marks.iter().map(Rect::centre).collect(),
        marks,
        pieces: joined,
    }
}

fn nearest(centres: &[f64], x: f64, reach: f64) -> Option<usize> {
    let after = centres.partition_point(|&c| c < x);
    [after.checked_sub(1), Some(after)]
        .into_iter()
        .flatten()
        .filter(|&i| i < centres.len() && (centres[i] - x).abs() <= reach)
        .min_by(|&a, &b| (centres[a] - x).abs().total_cmp(&(centres[b] - x).abs()))
}

fn walk(centres: &[f64], from: usize, pitch: f64, direction: f64) -> Vec<Place> {
    let (mut x, mut pitch) = (centres[from], pitch);
    let mut places: Vec<Place> = Vec::new();
    let mut empty_in_a_row = 0;
    while places.len() < GLYPHS - 1 {
        let expected = x + direction * pitch;
        let mark = nearest(centres, expected, WALK_REACH * pitch);
        if let Some(mark) = mark {
            pitch = (pitch + (centres[mark] - x).abs()) / 2.0;
            x = centres[mark];
            empty_in_a_row = 0;
        } else {
            empty_in_a_row += 1;
            if empty_in_a_row > MOST_EMPTY_PLACES {
                break;
            }
            x = expected;
        }
        places.push(Place { x, pitch, mark });
    }
    while places.last().is_some_and(|place| place.mark.is_none()) {
        places.pop();
    }
    places
}

fn walked_places(centres: &[f64], from: usize, pitch: f64) -> Vec<Place> {
    let mut places = walk(centres, from, pitch, -1.0);
    places.reverse();
    places.push(Place {
        x: centres[from],
        pitch,
        mark: Some(from),
    });
    places.extend(walk(centres, from, pitch, 1.0));
    places
}

fn line_through(points: &[(f64, f64)]) -> (f64, f64) {
    let n = points.len() as f64;
    let mean_k = points.iter().map(|p| p.0).sum::<f64>() / n;
    let mean_x = points.iter().map(|p| p.1).sum::<f64>() / n;
    let slope = points
        .iter()
        .map(|p| (p.0 - mean_k) * (p.1 - mean_x))
        .sum::<f64>()
        / points.iter().map(|p| (p.0 - mean_k).powi(2)).sum::<f64>();
    (mean_x - mean_k * slope, slope)
}

fn lined_places(centres: &[f64], anchor: f64, pitch: f64) -> Option<Vec<Place>> {
    let furthest = GLYPHS as i32 - 1;
    let (mut start, mut pitch) = (anchor, pitch);
    for span in [FIRST_FIT_SPAN, SECOND_FIT_SPAN, furthest] {
        let hits: Vec<(f64, f64)> = (-span..=span)
            .filter_map(|k| {
                let k = f64::from(k);
                nearest(centres, start + k * pitch, WALK_REACH * pitch).map(|i| (k, centres[i]))
            })
            .collect();
        let enough = if span == furthest {
            GLYPHS - MOST_EMPTY_PLACES
        } else {
            FEWEST_TO_FIT
        };
        if hits.len() < enough {
            return None;
        }
        (start, pitch) = line_through(&hits);
        if pitch <= 0.0 {
            return None;
        }
    }
    let places = (-furthest..=furthest).map(|k| {
        let x = start + f64::from(k) * pitch;
        Place {
            x,
            pitch,
            mark: nearest(centres, x, LINE_REACH * pitch),
        }
    });
    Some(places.collect())
}

fn skips_a_mark(run: &[Place], band: &Band) -> bool {
    let used: Vec<usize> = run.iter().filter_map(|place| place.mark).collect();
    let mut heights: Vec<i32> = used.iter().map(|&i| band.marks[i].height()).collect();
    heights.sort_unstable();
    let typical = heights[heights.len() / 2];
    (used[0]..=used[used.len() - 1]).any(|i| {
        TYPICAL.1 * band.marks[i].height() >= TYPICAL.0 * typical
            && run
                .iter()
                .all(|place| (band.centres[i] - place.x).abs() >= CELL_REACH * place.pitch)
    })
}

fn cell_at(place: &Place, band: &Band, empty: &Rect) -> Option<Rect> {
    if place.mark.is_none() {
        let half = EMPTY_CELL_HALF_WIDTH * place.pitch;
        return Some(Rect {
            left: ((place.x - half).round() as i32).max(empty.left),
            right: ((place.x + half).round() as i32).min(empty.right),
            ..*empty
        });
    }
    band.pieces
        .iter()
        .filter(|piece| (piece.centre() - place.x).abs() < CELL_REACH * place.pitch)
        .copied()
        .reduce(|cell, piece| cell.joined(&piece))
}

fn rows_on(places: &[Place], band: &Band, picture_width: i32) -> Vec<Row> {
    let mut rows = Vec::new();
    for run in places.windows(GLYPHS) {
        let marks: Vec<&Rect> = run
            .iter()
            .filter_map(|place| place.mark)
            .map(|i| &band.marks[i])
            .collect();
        if marks.len() < GLYPHS - MOST_EMPTY_PLACES || skips_a_mark(run, band) {
            continue;
        }
        let shortest = marks.iter().map(|m| m.height()).min().unwrap();
        let tallest = marks.iter().map(|m| m.height()).max().unwrap();
        if f64::from(shortest) < SHORTEST_IN_ROW * f64::from(tallest) {
            continue;
        }
        let empty = Rect {
            left: 0,
            top: marks.iter().map(|m| m.top).min().unwrap(),
            right: picture_width,
            bottom: marks.iter().map(|m| m.bottom).max().unwrap(),
        };
        let cells: Option<Vec<Rect>> = run
            .iter()
            .map(|place| cell_at(place, band, &empty))
            .collect();
        if let Some(cells) = cells.filter(|cells| cells.iter().all(|c| c.left < c.right)) {
            rows.push((
                marks.len(),
                Row {
                    cells: cells.try_into().unwrap(),
                    whole: marks.len() == GLYPHS,
                },
            ));
        }
    }
    let most = rows.iter().map(|(marks, _)| *marks).max().unwrap_or(0);
    rows.into_iter()
        .filter(|(marks, _)| *marks == most)
        .map(|(_, row)| row)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyph(left: i32) -> Rect {
        cell(left, left + 14)
    }

    fn cell(left: i32, right: i32) -> Rect {
        Rect {
            left,
            top: 100,
            right,
            bottom: 120,
        }
    }

    #[test]
    fn twelve_glyphs_at_one_pitch_are_a_row() {
        let row: Vec<Rect> = (0..12).map(|i| glyph(10 + 30 * i)).collect();
        let found = in_shapes(&row, 1000);
        assert_eq!(found.len(), 1);
        assert!(found[0].whole);
        assert_eq!(found[0].cells[0], cell(10, 24));
        assert_eq!(found[0].cells[11], cell(340, 354));
    }

    #[test]
    fn glyphs_out_of_step_break_the_row() {
        let mut row: Vec<Rect> = (0..12).map(|i| glyph(10 + 30 * i)).collect();
        row[6] = glyph(row[6].left + 12);
        row[3] = glyph(row[3].left - 12);
        row[9] = glyph(row[9].left + 12);
        assert!(in_shapes(&row, 1000).is_empty());
    }

    #[test]
    fn a_lost_glyph_gets_its_place_from_the_pitch() {
        let mut row: Vec<Rect> = (0..12).map(|i| glyph(10 + 30 * i)).collect();
        row.remove(5);
        let found = in_shapes(&row, 1000);
        assert!(
            found
                .iter()
                .any(|r| !r.whole && r.cells[5] == cell(155, 179)),
            "{found:?}"
        );
    }

    #[test]
    fn a_glyph_left_between_places_breaks_the_row() {
        let mut row: Vec<Rect> = (0..12).map(|i| glyph(10 + 30 * i)).collect();
        row.insert(6, glyph(10 + 30 * 5 + 15));
        assert!(in_shapes(&row, 1000).is_empty());
    }
}
