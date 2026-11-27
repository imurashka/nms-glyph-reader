use std::ops::Range;

use image::{GrayImage, Luma, RgbImage, imageops};

use super::text::{self, Drawn};
use crate::net::dot;
use crate::picture::Rect;

pub const LEFT_OF_STRIP: usize = 24;

const STRIP_WIDTH: f64 = 430.0;
const WIDTH: usize = LEFT_OF_STRIP + 450;
const ABOVE_STRIP_BOTTOM: usize = 4;
const TEXT_ROWS: usize = 24;
const NARROWEST_STRIP: f64 = 60.0;
const LEAST_LEFT_OF_STRIP: f64 = 4.0;
const BRIGHTEST: f64 = 0.997;
const INK_FROM: f32 = 0.3;
const FLAT: f32 = 1e-4;
const WORD_START: f32 = 0.15;

pub struct Line {
    pixels: Vec<f32>,
    width: usize,
    text_height: usize,
    sums: Vec<f64>,
    squares: Vec<f64>,
    left: f64,
    top: f64,
    scale: f64,
}

#[derive(Clone, Copy)]
pub struct Match {
    pub score: f32,
    pub size: usize,
    pub y: usize,
    pub x: usize,
    pub width: usize,
}

impl Match {
    pub const NONE: Match = Match {
        score: f32::NEG_INFINITY,
        size: 0,
        y: 0,
        x: 0,
        width: 0,
    };
}

fn darkest_channel_at(picture: &RgbImage, x: i64, y: i64) -> u8 {
    let inside = (0..i64::from(picture.width())).contains(&x)
        && (0..i64::from(picture.height())).contains(&y);
    if !inside {
        return 0;
    }
    let [r, g, b] = picture.get_pixel(x as u32, y as u32).0;
    r.min(g).min(b)
}

fn ink(grey: &[u8]) -> Vec<f32> {
    let mut sorted = grey.to_vec();
    sorted.sort_unstable();
    let at = |share: f64| f32::from(sorted[((sorted.len() - 1) as f64 * share).round() as usize]);
    let (background, brightest) = (at(0.5), at(BRIGHTEST));
    let none = background + INK_FROM * (brightest - background);
    let full = (brightest - none).max(1.0);
    grey.iter()
        .map(|&v| ((f32::from(v) - none) / full).clamp(0.0, 1.0))
        .collect()
}

impl Line {
    pub fn under(picture: &RgbImage, strip: &Rect, text_height: usize) -> Option<Line> {
        let [left, right, bottom] = [strip.left, strip.right, strip.bottom].map(f64::from);
        let scale = (right - left) / STRIP_WIDTH;
        if right - left < NARROWEST_STRIP || left < LEAST_LEFT_OF_STRIP * scale {
            return None;
        }
        let x0 = (left - LEFT_OF_STRIP as f64 * scale).round() as i64;
        let y0 = (bottom - ABOVE_STRIP_BOTTOM as f64 * scale).round() as i64;
        let height = TEXT_ROWS - 1 + text_height;
        let in_picture = GrayImage::from_fn(
            (WIDTH as f64 * scale).round().max(1.0) as u32,
            (height as f64 * scale).round().max(1.0) as u32,
            |x, y| {
                Luma([darkest_channel_at(
                    picture,
                    x0 + i64::from(x),
                    y0 + i64::from(y),
                )])
            },
        );
        let resized = imageops::resize(
            &in_picture,
            WIDTH as u32,
            height as u32,
            imageops::FilterType::Triangle,
        );
        let pixels = text::blurred(&ink(resized.as_raw()), WIDTH);
        Some(Line {
            left: x0 as f64,
            top: y0 as f64,
            scale,
            ..Line::new(pixels, WIDTH, text_height)
        })
    }

    pub fn in_picture(&self, found: &Match) -> Rect {
        let at = |from: f64, v: usize| (from + v as f64 * self.scale).round() as i32;
        Rect {
            left: at(self.left, found.x),
            top: at(self.top, found.y),
            right: at(self.left, found.x + found.width),
            bottom: at(self.top, found.y + self.text_height),
        }
    }

    fn new(pixels: Vec<f32>, width: usize, text_height: usize) -> Line {
        let height = pixels.len() / width;
        let mut sums = vec![0.0; (width + 1) * (height + 1)];
        let mut squares = sums.clone();
        for y in 0..height {
            let (mut sum, mut square) = (0.0, 0.0);
            for x in 0..width {
                let v = f64::from(pixels[y * width + x]);
                sum += v;
                square += v * v;
                let at = (y + 1) * (width + 1) + x + 1;
                sums[at] = sums[at - width - 1] + sum;
                squares[at] = squares[at - width - 1] + square;
            }
        }
        Line {
            pixels,
            width,
            text_height,
            sums,
            squares,
            left: 0.0,
            top: 0.0,
            scale: 1.0,
        }
    }

    pub fn halved(&self) -> Line {
        let (w, h) = (self.width / 2, self.pixels.len() / self.width / 2);
        let mut out = vec![0.0; w * h];
        for y in 0..h {
            let (above, below) = (2 * y * self.width, (2 * y + 1) * self.width);
            for x in 0..w {
                out[y * w + x] = (self.pixels[above + 2 * x]
                    + self.pixels[above + 2 * x + 1]
                    + self.pixels[below + 2 * x]
                    + self.pixels[below + 2 * x + 1])
                    * 0.25;
            }
        }
        Line::new(out, w, self.text_height / 2)
    }

    pub fn rows(&self) -> usize {
        self.pixels.len() / self.width - self.text_height + 1
    }

    pub fn columns(&self, drawn: &Drawn) -> usize {
        (self.width + 1).saturating_sub(drawn.width)
    }

    fn window(&self, y: usize, x: usize, width: usize) -> (f32, f32) {
        let line = self.width + 1;
        let (top, bottom) = (y * line, (y + self.text_height) * line);
        let inside = |table: &[f64]| {
            table[bottom + x + width] - table[bottom + x] - table[top + x + width] + table[top + x]
        };
        (inside(&self.sums) as f32, inside(&self.squares) as f32)
    }

    fn likeness(&self, drawn: &Drawn, y: usize, x: usize) -> f32 {
        let n = (self.text_height * drawn.width) as f32;
        let (sum, squares) = self.window(y, x, drawn.width);
        let spread = squares - sum * sum / n;
        let drawn_spread = drawn.squares - drawn.sum * drawn.sum / n;
        if spread <= n * FLAT || drawn_spread <= 0.0 {
            return 0.0;
        }
        let mut product = 0.0;
        for line in 0..self.text_height {
            let at = (y + line) * self.width + x;
            product += dot::product(
                &self.pixels[at..at + drawn.width],
                &drawn.pixels[line * drawn.width..(line + 1) * drawn.width],
            );
        }
        (product - sum * drawn.sum / n) / (spread * drawn_spread).sqrt()
    }

    pub fn best(&self, drawn: &Drawn, rows: Range<usize>, columns: Range<usize>) -> Match {
        let mut best = Match::NONE;
        for y in rows {
            for x in columns.clone() {
                let score = self.likeness(drawn, y, x);
                if score > best.score {
                    best = Match {
                        score,
                        size: drawn.size,
                        y,
                        x,
                        width: drawn.width,
                    };
                }
            }
        }
        best
    }

    pub fn word_starts(&self, rows: Range<usize>, columns: usize) -> Vec<usize> {
        let ink: Vec<f32> = (0..columns)
            .map(|x| {
                rows.clone()
                    .map(|y| self.pixels[y * self.width + x])
                    .fold(0.0, f32::max)
            })
            .collect();
        let rises = |x: usize| x > 0 && ink[x] - ink[x - 1] >= WORD_START;
        (1..columns)
            .filter(|&x| rises(x) && !rises(x - 1))
            .collect()
    }
}
