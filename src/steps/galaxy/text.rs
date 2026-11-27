use std::ops::RangeInclusive;
use std::sync::OnceLock;

use ab_glyph::{Font, FontRef, OutlinedGlyph, PxScale, ScaleFont, point};

use super::names::{NAMES, TRANSLATED};

pub const TEXT_HEIGHT: f32 = 16.2;
pub const SIZES: [f32; 13] = [
    0.88, 0.9, 0.92, 0.94, 0.96, 0.98, 1.0, 1.02, 1.04, 1.06, 1.08, 1.1, 1.12,
];
const HALF_SIZES: RangeInclusive<usize> = 4..=8;
pub const PAD: usize = 4;

const ROBOTO: &[u8] = include_bytes!("../../assets/Roboto-Regular.ttf");
const FULL_SIZE: usize = 6;
const NARROWEST: usize = 56;
const BLUR_SIGMA: f32 = 0.55;

pub struct Lettering {
    pub pixels: Vec<f32>,
    pub width: usize,
    pub height: usize,
    pub ascent: f32,
}

pub struct Drawn {
    pub pixels: Vec<f32>,
    pub width: usize,
    pub size: usize,
    pub sum: f32,
    pub squares: f32,
}

pub struct Texts {
    pub font: FontRef<'static>,
    pub font_sizes: Vec<f32>,
    pub height: usize,
    pub names: Vec<(usize, &'static str)>,
    pub half_size: Vec<Vec<Drawn>>,
    pub mean: Drawn,
}

fn outlines(font: &FontRef, text: &str, font_size: f32) -> Vec<OutlinedGlyph> {
    let scaled = font.as_scaled(PxScale::from(font_size));
    let mut x = 0.0;
    let mut previous = None;
    let mut outlines = Vec::new();
    for letter in text.chars() {
        let id = scaled.glyph_id(letter);
        if let Some(before) = previous {
            x += scaled.kern(before, id);
        }
        let glyph = id.with_scale_and_position(font_size, point(x, 0.0));
        x += scaled.h_advance(id);
        previous = Some(id);
        outlines.extend(font.outline_glyph(glyph));
    }
    outlines
}

pub fn render(font: &FontRef, text: &str, font_size: f32) -> Lettering {
    let outlines = outlines(font, text, font_size);
    let bounds: Vec<_> = outlines.iter().map(OutlinedGlyph::px_bounds).collect();
    let left = bounds.iter().map(|b| b.min.x).fold(f32::INFINITY, f32::min);
    let top = bounds.iter().map(|b| b.min.y).fold(f32::INFINITY, f32::min);
    let right = bounds
        .iter()
        .map(|b| b.max.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let bottom = bounds
        .iter()
        .map(|b| b.max.y)
        .fold(f32::NEG_INFINITY, f32::max);
    let (width, height) = ((right - left) as usize, (bottom - top) as usize);
    let mut pixels = vec![0.0f32; width * height];
    for (outline, b) in outlines.iter().zip(&bounds) {
        let (dx, dy) = ((b.min.x - left) as usize, (b.min.y - top) as usize);
        outline.draw(|x, y, coverage| {
            let (x, y) = (dx + x as usize, dy + y as usize);
            if x < width && y < height {
                let pixel = &mut pixels[y * width + x];
                *pixel = pixel.max(coverage);
            }
        });
    }
    Lettering {
        pixels,
        width,
        height,
        ascent: -top,
    }
}

fn blur_weights() -> Vec<f32> {
    let reach = (3.0 * BLUR_SIGMA).ceil() as i32;
    let weights: Vec<f32> = (-reach..=reach)
        .map(|i| (-0.5 * (i as f32 / BLUR_SIGMA).powi(2)).exp())
        .collect();
    let total: f32 = weights.iter().sum();
    weights.into_iter().map(|w| w / total).collect()
}

fn blur_pass(source: &[f32], weights: &[f32], width: usize, down: bool) -> Vec<f32> {
    let height = source.len() / width;
    let reach = weights.len() / 2;
    let mut out = vec![0.0; source.len()];
    let mut i = 0;
    while i < source.len() {
        let (position, count, step) = if down {
            (i / width, height, width)
        } else {
            (i % width, width, 1)
        };
        let mut total = 0.0;
        let mut k = 0;
        while k < weights.len() {
            if position + k >= reach && position + k - reach < count {
                total += weights[k] * source[i + k * step - reach * step];
            }
            k += 1;
        }
        out[i] = total;
        i += 1;
    }
    out
}

pub fn blurred(pixels: &[f32], width: usize) -> Vec<f32> {
    let weights = blur_weights();
    let along_lines = blur_pass(pixels, &weights, width, false);
    blur_pass(&along_lines, &weights, width, true)
}

fn drawing(pixels: Vec<f32>, width: usize, size: usize) -> Drawn {
    let (mut sum, mut squares) = (0.0, 0.0);
    let mut i = 0;
    while i < pixels.len() {
        sum += pixels[i];
        squares += pixels[i] * pixels[i];
        i += 1;
    }
    Drawn {
        pixels,
        width,
        size,
        sum,
        squares,
    }
}

fn padded(ink: &Lettering, pad: usize, height: usize, narrowest: usize) -> (Vec<f32>, usize) {
    let width = (ink.width + 2 * pad).max(narrowest);
    let mut canvas = vec![0.0; height * width];
    for y in 0..ink.height.min(height) {
        canvas[y * width + pad..y * width + pad + ink.width]
            .copy_from_slice(&ink.pixels[y * ink.width..(y + 1) * ink.width]);
    }
    (canvas, width)
}

fn stretched(drawn: &Drawn, height: usize, factor: f32, size: usize) -> Drawn {
    let width = (drawn.width as f32 * factor).ceil() as usize;
    let mut pixels = vec![0.0; height * width];
    let at = |y: usize, x: usize| {
        if y < height && x < drawn.width {
            drawn.pixels[y * drawn.width + x]
        } else {
            0.0
        }
    };
    for y in 0..height {
        let from_y = y as f32 / factor;
        let (y0, fy) = (from_y as usize, from_y.fract());
        for x in 0..width {
            let from_x = x as f32 / factor;
            let (x0, fx) = (from_x as usize, from_x.fract());
            let above = at(y0, x0) * (1.0 - fx) + at(y0, x0 + 1) * fx;
            let below = at(y0 + 1, x0) * (1.0 - fx) + at(y0 + 1, x0 + 1) * fx;
            pixels[y * width + x] = above * (1.0 - fy) + below * fy;
        }
    }
    drawing(pixels, width, size)
}

fn mean(drawings: &[Drawn], height: usize) -> Drawn {
    let width = drawings.iter().map(|d| d.width).max().unwrap();
    let mut pixels = vec![0.0; height * width];
    let share = 1.0 / drawings.len() as f32;
    for drawn in drawings {
        for y in 0..height {
            for x in 0..drawn.width {
                pixels[y * width + x] += drawn.pixels[y * drawn.width + x] * share;
            }
        }
    }
    drawing(pixels, width, FULL_SIZE)
}

pub fn texts() -> &'static Texts {
    static TEXTS: OnceLock<Texts> = OnceLock::new();
    TEXTS.get_or_init(|| {
        let font = FontRef::try_from_slice(ROBOTO).unwrap();
        let ascent_of_one = render(&font, "Galaxy", 100.0).ascent / 100.0;
        let font_sizes: Vec<f32> = SIZES
            .iter()
            .map(|size| TEXT_HEIGHT * size / ascent_of_one)
            .collect();
        let names: Vec<(usize, &str)> = NAMES
            .iter()
            .copied()
            .enumerate()
            .chain(TRANSLATED)
            .collect();
        let largest = font_sizes[font_sizes.len() - 1];
        let height = names
            .iter()
            .map(|(_, text)| render(&font, text, largest).height)
            .max()
            .unwrap();
        let half: Vec<Drawn> = names
            .iter()
            .map(|(_, text)| {
                let ink = render(&font, text, font_sizes[FULL_SIZE] / 2.0);
                let (pixels, width) = padded(&ink, PAD / 2, height / 2, NARROWEST / 2);
                drawing(pixels, width, FULL_SIZE)
            })
            .collect();
        let half_size = half
            .iter()
            .map(|full| {
                HALF_SIZES
                    .map(|size| stretched(full, height / 2, SIZES[size] / SIZES[FULL_SIZE], size))
                    .collect()
            })
            .collect();
        Texts {
            mean: mean(&half, height / 2),
            font,
            font_sizes,
            height,
            names,
            half_size,
        }
    })
}

impl Texts {
    pub fn drawn(&self, text: &str, size: usize) -> Drawn {
        let ink = render(&self.font, text, self.font_sizes[size]);
        let narrowest = (NARROWEST as f32 * SIZES[size]) as usize;
        let (canvas, width) = padded(&ink, PAD, self.height, narrowest);
        drawing(blurred(&canvas, width), width, size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tallest_name_is_at_the_largest_size() {
        let texts = texts();
        let tallest = texts
            .names
            .iter()
            .flat_map(|(_, text)| {
                texts
                    .font_sizes
                    .iter()
                    .map(|&size| render(&texts.font, text, size).height)
            })
            .max()
            .unwrap();
        assert_eq!(texts.height, tallest);
    }
}
