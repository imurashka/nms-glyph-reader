use crate::picture::{Mask, resize};

pub const SIDE: usize = 32;
pub const DIGITS: &[u8; 16] = b"0123456789ABCDEF";

const BLUR_SIGMA: f64 = 0.8;

fn blurred(cell: &[f32]) -> Vec<f32> {
    let edge = (-0.5 / (BLUR_SIGMA * BLUR_SIGMA)).exp() as f32;
    let total = 1.0 / (f64::from(edge) * 2.0 + 1.0);
    let (side, centre) = ((f64::from(edge) * total) as f32, total as f32);
    let mirrored = |i: usize, step: isize| match i as isize + step {
        -1 => 1,
        n if n == SIDE as isize => SIDE - 2,
        n => n as usize,
    };
    let pass = |source: &[f32], along_lines: bool| {
        let mut out = vec![0.0; SIDE * SIDE];
        for y in 0..SIDE {
            for x in 0..SIDE {
                let pixel = |step: isize| {
                    if along_lines {
                        source[y * SIDE + mirrored(x, step)]
                    } else {
                        source[mirrored(y, step) * SIDE + x]
                    }
                };
                out[y * SIDE + x] = pixel(0) * centre + (pixel(-1) + pixel(1)) * side;
            }
        }
        out
    };
    pass(&pass(cell, true), false)
}

fn ink_square(cell: &Mask) -> Option<(Vec<f32>, usize)> {
    let (w, h) = (cell.width, cell.height());
    let ink = |x: usize, y: usize| cell.pixels[y * w + x];
    let left = (0..w).find(|&x| (0..h).any(|y| ink(x, y)))?;
    let right = (0..w).rfind(|&x| (0..h).any(|y| ink(x, y)))?;
    let top = (0..h).find(|&y| (0..w).any(|x| ink(x, y)))?;
    let bottom = (0..h).rfind(|&y| (0..w).any(|x| ink(x, y)))?;
    let (ink_w, ink_h) = (right - left + 1, bottom - top + 1);
    let side = ink_w.max(ink_h);
    let (dx, dy) = ((side - ink_w) / 2, (side - ink_h) / 2);
    let mut square = vec![0.0f32; side * side];
    for y in 0..ink_h {
        for x in 0..ink_w {
            if ink(left + x, top + y) {
                square[(dy + y) * side + dx + x] = 1.0;
            }
        }
    }
    Some((square, side))
}

pub fn normalise(cell: &Mask) -> Option<Vec<f32>> {
    let (square, side) = ink_square(cell)?;
    let mut v = blurred(&resize::square(&square, side, SIDE));
    let mean = (v.iter().map(|&x| f64::from(x)).sum::<f64>() / v.len() as f64) as f32;
    v.iter_mut().for_each(|x| *x -= mean);
    let length = v
        .iter()
        .map(|&x| f64::from(x) * f64::from(x))
        .sum::<f64>()
        .sqrt() as f32;
    if length == 0.0 {
        return None;
    }
    v.iter_mut().for_each(|x| *x /= length);
    Some(v)
}

pub fn probabilities(cell: &Mask) -> Option<Vec<f32>> {
    normalise(cell).map(|v| super::net().probabilities(&v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_font_glyph_reads_as_its_digit() {
        for (i, d) in DIGITS.iter().enumerate() {
            let path = format!(
                "{}/tests/font/font_{}.png",
                env!("CARGO_MANIFEST_DIR"),
                *d as char
            );
            let glyph = image::open(&path).unwrap().into_luma8();
            let cell = Mask {
                pixels: glyph.pixels().map(|p| p.0[0] > 127).collect(),
                width: glyph.width() as usize,
            };
            let p = probabilities(&cell).unwrap();
            assert!(p[i] > 0.99, "font_{}: {p:?}", *d as char);
        }
    }
}
