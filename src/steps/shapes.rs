use image::RgbImage;

use crate::picture::{Mask, Rect, decode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ink {
    Above(u8),
    Lighter,
    Darker,
}

pub const LEVELS: [Ink; 3] = [Ink::Above(210), Ink::Above(170), Ink::Above(130)];
pub const LOCAL: [Ink; 2] = [Ink::Lighter, Ink::Darker];
pub const ALL: [Ink; 5] = [LEVELS[0], LEVELS[1], LEVELS[2], LOCAL[0], LOCAL[1]];

pub const MOST_SHAPES: usize = 19_999;
const SMALLEST_PICTURE: u32 = 200;
const LOCAL_CONTRAST: i32 = 40;
const CHANNELS: i32 = 3;
const SURROUNDINGS: usize = 40;
const LEAST_SURROUNDINGS: usize = 6;

pub struct Layer {
    pub ink: Ink,
    pub scale: f64,
    pub mask: Mask,
    pub shapes: Vec<Rect>,
}

pub fn cut(picture: &RgbImage, inks: &[Ink]) -> Vec<Layer> {
    if picture.width() < SMALLEST_PICTURE && picture.height() < SMALLEST_PICTURE {
        return Vec::new();
    }
    let shrunk = decode::shrunk(picture);
    let mut layers = Vec::new();
    for view in std::iter::once(picture).chain(shrunk.as_ref()) {
        let scale = f64::from(picture.width()) / f64::from(view.width());
        let darkest = darkest_channel(view);
        for &ink in inks {
            let mask = mask(view, &darkest, ink);
            if let Some(shapes) = find(&mask, MOST_SHAPES) {
                layers.push(Layer {
                    ink,
                    scale,
                    mask,
                    shapes,
                });
            }
        }
    }
    layers
}

pub fn mask(picture: &RgbImage, darkest: &[u8], ink: Ink) -> Mask {
    let pixels = match ink {
        Ink::Above(level) => above(darkest, level),
        Ink::Lighter => standing_out(picture, 1),
        Ink::Darker => standing_out(picture, -1),
    };
    Mask {
        pixels,
        width: picture.width() as usize,
    }
}

pub fn darkest_channel(picture: &RgbImage) -> Vec<u8> {
    let raw = picture.as_raw();
    let mut out = vec![0u8; raw.len() / 3];
    let mut i = 0;
    while i < out.len() {
        let (r, g, b) = (raw[3 * i], raw[3 * i + 1], raw[3 * i + 2]);
        let low = if r < g { r } else { g };
        out[i] = if b < low { b } else { low };
        i += 1;
    }
    out
}

fn above(darkest: &[u8], level: u8) -> Vec<bool> {
    let mut out = vec![false; darkest.len()];
    let mut i = 0;
    while i < darkest.len() {
        out[i] = darkest[i] >= level;
        i += 1;
    }
    out
}

fn add_line(columns: &mut [i32], line: &[i32], sign: i32) {
    let mut x = 0;
    while x < columns.len() {
        columns[x] += sign * line[x];
        x += 1;
    }
}

fn standing_out(picture: &RgbImage, sign: i32) -> Vec<bool> {
    let (w, h) = (picture.width() as usize, picture.height() as usize);
    let raw = picture.as_raw();
    let mut light = vec![0i32; w * h];
    let mut i = 0;
    while i < light.len() {
        light[i] = i32::from(raw[3 * i]) + i32::from(raw[3 * i + 1]) + i32::from(raw[3 * i + 2]);
        i += 1;
    }
    let reach = (h.min(w) / SURROUNDINGS).max(LEAST_SURROUNDINGS);
    let window = |i: usize, n: usize| ((i + reach + 1).min(n) - i.saturating_sub(reach)) as i32;
    let window_widths: Vec<i32> = (0..w).map(|x| window(x, w)).collect();
    let mut columns = vec![0i32; w];
    for y in 0..reach.min(h) {
        add_line(&mut columns, &light[y * w..(y + 1) * w], 1);
    }
    let mut out = vec![false; w * h];
    let mut y = 0;
    while y < h {
        if y + reach < h {
            add_line(
                &mut columns,
                &light[(y + reach) * w..(y + reach + 1) * w],
                1,
            );
        }
        if y > reach {
            add_line(
                &mut columns,
                &light[(y - reach - 1) * w..(y - reach) * w],
                -1,
            );
        }
        let window_height = window(y, h);
        let mut around = 0;
        let mut x = 0;
        while x < reach.min(w) {
            around += columns[x];
            x += 1;
        }
        let mut x = 0;
        while x < w {
            if x + reach < w {
                around += columns[x + reach];
            }
            if x > reach {
                around -= columns[x - reach - 1];
            }
            let i = y * w + x;
            let area = window_widths[x] * window_height;
            out[i] = sign * (light[i] * area - around) > CHANNELS * LOCAL_CONTRAST * area;
            x += 1;
        }
        y += 1;
    }
    out
}

struct Run {
    y: u32,
    first: u32,
    last: u32,
}

fn root(parent: &mut [u32], mut label: u32) -> u32 {
    while parent[label as usize] != label {
        let up = parent[label as usize];
        parent[label as usize] = parent[up as usize];
        label = up;
    }
    label
}

fn joined_runs(mask: &Mask) -> (Vec<Run>, Vec<u32>) {
    let (w, h) = (mask.width, mask.height());
    let mut runs: Vec<Run> = Vec::new();
    let mut parent: Vec<u32> = Vec::new();
    let mut line_above = 0..0;
    let mut y = 0;
    while y < h {
        let line = &mask.pixels[y * w..(y + 1) * w];
        let line_start = runs.len();
        let mut above = line_above.start;
        let mut x = 0;
        while x < w {
            if !line[x] {
                x += 1;
                continue;
            }
            let first = x as u32;
            while x < w && line[x] {
                x += 1;
            }
            let last = x as u32 - 1;
            let mut label = runs.len() as u32;
            parent.push(label);
            while above < line_above.end && runs[above].last + 1 < first {
                above += 1;
            }
            let mut touching = above;
            while touching < line_above.end && runs[touching].first <= last + 1 {
                let other = root(&mut parent, touching as u32);
                if other != label {
                    let (low, high) = (other.min(label), other.max(label));
                    parent[high as usize] = low;
                    label = low;
                }
                touching += 1;
            }
            runs.push(Run {
                y: y as u32,
                first,
                last,
            });
        }
        line_above = line_start..runs.len();
        y += 1;
    }
    (runs, parent)
}

pub fn find(mask: &Mask, most: usize) -> Option<Vec<Rect>> {
    let (runs, mut parent) = joined_runs(mask);
    let mut shape_of = vec![u32::MAX; runs.len()];
    let mut shapes: Vec<Rect> = Vec::new();
    let mut i = 0;
    while i < runs.len() {
        let run = &runs[i];
        let (y, first, last) = (run.y as i32, run.first as i32, run.last as i32);
        let root = root(&mut parent, i as u32) as usize;
        if shape_of[root] == u32::MAX {
            if shapes.len() == most {
                return None;
            }
            shape_of[root] = shapes.len() as u32;
            shapes.push(Rect {
                left: first,
                top: y,
                right: last + 1,
                bottom: y + 1,
            });
        }
        let shape = &mut shapes[shape_of[root] as usize];
        if first < shape.left {
            shape.left = first;
        }
        if last + 1 > shape.right {
            shape.right = last + 1;
        }
        shape.bottom = y + 1;
        i += 1;
    }
    Some(shapes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drawn(lines: &[&str]) -> Mask {
        Mask {
            pixels: lines
                .iter()
                .flat_map(|line| line.bytes().map(|b| b == b'#'))
                .collect(),
            width: lines[0].len(),
        }
    }

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect {
            left,
            top,
            right,
            bottom,
        }
    }

    fn flooded(mask: &Mask) -> Vec<Rect> {
        let (w, h) = (mask.width as i32, mask.height() as i32);
        let mut seen = vec![false; mask.pixels.len()];
        let mut shapes = Vec::new();
        for start in 0..mask.pixels.len() {
            if !mask.pixels[start] || seen[start] {
                continue;
            }
            seen[start] = true;
            let mut todo = vec![start as i32];
            let mut shape = rect(i32::MAX, i32::MAX, 0, 0);
            while let Some(i) = todo.pop() {
                let (x, y) = (i % w, i / w);
                shape = shape.joined(&rect(x, y, x + 1, y + 1));
                for (nx, ny) in (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (x + dx, y + dy))) {
                    let n = (ny * w + nx) as usize;
                    if (0..w).contains(&nx) && (0..h).contains(&ny) && mask.pixels[n] && !seen[n] {
                        seen[n] = true;
                        todo.push(n as i32);
                    }
                }
            }
            shapes.push(shape);
        }
        shapes
    }

    #[test]
    fn local_ink_is_what_stands_out_from_its_surroundings() {
        let mut picture = RgbImage::from_pixel(200, 200, image::Rgb([150, 160, 170]));
        for y in 90..110 {
            for x in 90..94 {
                picture.put_pixel(x, y, image::Rgb([250, 250, 250]));
                picture.put_pixel(x + 50, y, image::Rgb([30, 30, 30]));
            }
        }
        let at = |x: usize, y: usize| y * 200 + x;
        let darkest = darkest_channel(&picture);
        let lighter = mask(&picture, &darkest, Ink::Lighter).pixels;
        let darker = mask(&picture, &darkest, Ink::Darker).pixels;
        assert!(lighter[at(92, 100)] && !lighter[at(142, 100)] && !lighter[at(20, 20)]);
        assert!(darker[at(142, 100)] && !darker[at(92, 100)] && !darker[at(20, 20)]);
        assert!(mask(&picture, &darkest, Ink::Above(130)).pixels[at(20, 20)]);
    }

    #[test]
    fn pixels_touching_at_a_corner_are_one_shape() {
        let mask = drawn(&["...#....", "#...#...", ".#...#..", "......#."]);
        let found = find(&mask, MOST_SHAPES).unwrap();
        assert_eq!(found, vec![rect(3, 0, 7, 4), rect(0, 1, 2, 3)]);
    }

    #[test]
    fn shapes_are_the_same_as_a_flood_fill_finds() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        for size in [(1, 1), (7, 5), (40, 30), (64, 3), (3, 64)] {
            for ink_in_16 in [2, 6, 8, 11, 15] {
                let pixels = (0..size.0 * size.1)
                    .map(|_| {
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        seed % 16 < ink_in_16
                    })
                    .collect();
                let mask = Mask {
                    pixels,
                    width: size.0,
                };
                let mut found = find(&mask, MOST_SHAPES).unwrap();
                let mut expected = flooded(&mask);
                found.sort();
                expected.sort();
                assert_eq!(found, expected, "{size:?}, ink {ink_in_16}/16");
            }
        }
    }

    #[test]
    fn too_many_shapes_give_none() {
        let mask = drawn(&["#.#.#.#"]);
        assert!(find(&mask, 4).is_some());
        assert!(find(&mask, 3).is_none());
    }
}
