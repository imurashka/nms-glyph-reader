mod line;
mod names;
mod text;

use std::ops::Range;

use image::RgbImage;

use crate::picture::Rect;
use line::{LEFT_OF_STRIP, Line, Match};
pub use names::NAMES;
use text::{Drawn, PAD, SIZES, TEXT_HEIGHT, Texts};

const TEXT_STARTS_WITHIN: usize = LEFT_OF_STRIP / 2 + 3;
const WORDS_START_WITHIN: usize = (LEFT_OF_STRIP + 180) / 2;
const CHECKED_EXACTLY: usize = 24;
const ROUGH_COLUMNS_REACH: usize = 1;
const EXACT_SIZES_REACH: usize = 1;
const ROUGH_ROWS_REACH: usize = 1;
const EXACT_ROWS_REACH: usize = 3;
const EXACT_COLUMNS_REACH: usize = 3;
const LABEL_SIZES_REACH: usize = 3;
const LABEL_PLACE_REACH: usize = 2;
const SURE_SCORE: f32 = 0.8;
const LEAST_SCORE: f32 = 0.65;
const LABEL_CONFIRMS: f32 = 0.65;
const LEAD_OVER_SECOND: f32 = 0.04;

pub fn name(picture: &RgbImage, strip: &Rect) -> Option<(&'static str, Rect)> {
    let texts = text::texts();
    let line = Line::under(picture, strip, texts.height)?;
    let rough = rough_matches(&line.halved(), texts);
    let mut exact = vec![Match::NONE; NAMES.len()];
    for (text, rough) in rough.into_iter().take(CHECKED_EXACTLY) {
        let (galaxy, name) = texts.names[text];
        for size in near(rough.size, EXACT_SIZES_REACH, SIZES.len()) {
            let drawn = texts.drawn(name, size);
            let rows = near(2 * rough.y, EXACT_ROWS_REACH, line.rows());
            let columns = near(2 * rough.x, EXACT_COLUMNS_REACH, line.columns(&drawn));
            let found = line.best(&drawn, rows, columns);
            if found.score > exact[galaxy].score {
                exact[galaxy] = found;
            }
        }
    }
    let mut order: Vec<usize> = (0..NAMES.len()).collect();
    order.sort_by(|&a, &b| exact[b].score.total_cmp(&exact[a].score));
    let (galaxy, best, second) = (order[0], exact[order[0]], exact[order[1]]);
    let clear = best.score >= LEAST_SCORE && best.score - second.score >= LEAD_OVER_SECOND;
    let given = clear
        && (best.score >= SURE_SCORE || label_score(&line, texts, galaxy, best) >= LABEL_CONFIRMS);
    given.then(|| (NAMES[galaxy], line.in_picture(&best)))
}

fn near(at: usize, reach: usize, end: usize) -> Range<usize> {
    at.saturating_sub(reach)..(at + reach + 1).min(end)
}

fn rough_matches(half: &Line, texts: &Texts) -> Vec<(usize, Match)> {
    let text_row = half
        .best(
            &texts.mean,
            0..half.rows(),
            0..TEXT_STARTS_WITHIN.min(half.columns(&texts.mean)),
        )
        .y;
    let rows = near(text_row, ROUGH_ROWS_REACH, half.rows());
    let letters = text_row..text_row + (TEXT_HEIGHT / 2.0) as usize;
    let word_starts = half.word_starts(letters, WORDS_START_WITHIN);
    let mut matches: Vec<(usize, Match)> = texts
        .half_size
        .iter()
        .map(|sizes| rough_match(half, sizes, &rows, &word_starts))
        .enumerate()
        .collect();
    matches.sort_by(|a, b| b.1.score.total_cmp(&a.1.score));
    matches
}

fn rough_match(half: &Line, sizes: &[Drawn], rows: &Range<usize>, word_starts: &[usize]) -> Match {
    let mut best = Match::NONE;
    for drawn in sizes {
        for &start in word_starts {
            let columns = near(
                start.saturating_sub(PAD / 2),
                ROUGH_COLUMNS_REACH,
                half.columns(drawn),
            );
            let found = half.best(drawn, rows.clone(), columns);
            if found.score > best.score {
                best = found;
            }
        }
    }
    best
}

fn label_score(line: &Line, texts: &Texts, galaxy: usize, name: Match) -> f32 {
    if NAMES[galaxy].ends_with("Dimension") {
        return f32::NEG_INFINITY;
    }
    let label = format!("{} Galaxy", NAMES[galaxy]);
    near(name.size, LABEL_SIZES_REACH, SIZES.len())
        .map(|size| {
            let drawn = texts.drawn(&label, size);
            let rows = near(name.y, LABEL_PLACE_REACH, line.rows());
            let columns = near(name.x, LABEL_PLACE_REACH, line.columns(&drawn));
            line.best(&drawn, rows, columns).score
        })
        .fold(f32::NEG_INFINITY, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture_with(line: &str, size: f32, strip: &Rect) -> RgbImage {
        let texts = text::texts();
        let scale = strip.width() as f32 / 430.0;
        let font_size = texts.font_sizes[6] * size * scale;
        let ink = text::render(&texts.font, line, font_size);
        let mut picture = RgbImage::from_pixel(1400, 400, image::Rgb([24, 26, 30]));
        let (left, top) = (
            strip.left as usize,
            strip.bottom as usize + (8.0 * scale) as usize,
        );
        for y in 0..ink.height {
            for x in 0..ink.width {
                let light = 24.0 + 210.0 * ink.pixels[y * ink.width + x];
                let pixel = image::Rgb([light as u8; 3]);
                picture.put_pixel((left + x) as u32, (top + y) as u32, pixel);
            }
        }
        picture
    }

    fn strip(width: i32) -> Rect {
        Rect {
            left: 100,
            top: 40,
            right: 100 + width,
            bottom: 100,
        }
    }

    fn read(line: &str, size: f32, strip_width: i32) -> Option<&'static str> {
        let strip = strip(strip_width);
        name(&picture_with(line, size, &strip), &strip).map(|(galaxy, _)| galaxy)
    }

    #[test]
    fn the_area_of_a_name_holds_the_drawn_name() {
        for strip_width in [430, 860] {
            let strip = strip(strip_width);
            let scale = strip_width as f32 / 430.0;
            let (_, area) = name(&picture_with("Euclid Galaxy", 1.0, &strip), &strip).unwrap();
            let text_top = strip.bottom + (8.0 * scale) as i32;
            assert!(
                (area.left - strip.left).abs() <= (6.0 * scale) as i32,
                "{area:?}"
            );
            assert!(
                (area.top - text_top).abs() <= (6.0 * scale) as i32,
                "{area:?}"
            );
            let euclid_width = (60.0 * scale) as i32;
            assert!(
                area.width() >= euclid_width && area.width() <= 2 * euclid_width,
                "{area:?}"
            );
        }
    }

    #[test]
    fn a_name_reads_at_the_smallest_and_the_largest_text_size_searched() {
        for strip_width in [430, 860] {
            for size in [0.94, 1.0, 1.06] {
                let read = |line| read(line, size, strip_width);
                assert_eq!(read("Eissentam Galaxy"), Some("Eissentam"), "{size}");
                assert_eq!(
                    read("Hilbert Dimension"),
                    Some("Hilbert Dimension"),
                    "{size}"
                );
                assert_eq!(read("Galaxie Euclide"), Some("Euclid"), "{size}");
            }
        }
    }

    #[test]
    fn a_line_without_a_galaxy_name_reads_nothing() {
        assert_eq!(read("Somewhere Else Entirely", 1.0, 430), None);
        assert_eq!(read("", 1.0, 430), None);
    }
}
