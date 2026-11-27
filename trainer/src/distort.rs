use crate::SIDE;
use crate::net::Random;

const CHANCE_CLEAN: f32 = 0.2;
const MOST_SHIFT: f32 = 1.5;
const MOST_TURN: f32 = 0.07;
const STROKE_CHANCE: f32 = 0.35;
const EXTRA_STROKE_STEP_CHOICES: u64 = 3;
const BLUR_CHANCE: f32 = 0.3;
const MOST_NOISE: f32 = 0.3;

fn between(random: &mut Random, low: f32, high: f32) -> f32 {
    low + (high - low) * random.unit()
}

fn warp(cell: &[f32], random: &mut Random) -> Vec<f32> {
    let background = cell.iter().fold(f32::MAX, |m, &v| m.min(v));
    let zoom = between(random, 0.88, 1.08);
    let turn = between(random, -MOST_TURN, MOST_TURN);
    let (shift_x, shift_y) = (
        between(random, -MOST_SHIFT, MOST_SHIFT),
        between(random, -MOST_SHIFT, MOST_SHIFT),
    );
    let (sin, cos) = turn.sin_cos();
    let centre = (SIDE as f32 - 1.0) / 2.0;
    let at = |x: isize, y: isize| {
        if x < 0 || y < 0 || x >= SIDE as isize || y >= SIDE as isize {
            background
        } else {
            cell[y as usize * SIDE + x as usize]
        }
    };
    let mut out = Vec::with_capacity(SIDE * SIDE);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (dx, dy) = (x as f32 - centre - shift_x, y as f32 - centre - shift_y);
            let sx = (cos * dx + sin * dy) / zoom + centre;
            let sy = (-sin * dx + cos * dy) / zoom + centre;
            let (x0, y0) = (sx.floor(), sy.floor());
            let (fx, fy) = (sx - x0, sy - y0);
            let (x0, y0) = (x0 as isize, y0 as isize);
            let top = at(x0, y0) * (1.0 - fx) + at(x0 + 1, y0) * fx;
            let bottom = at(x0, y0 + 1) * (1.0 - fx) + at(x0 + 1, y0 + 1) * fx;
            out.push(top * (1.0 - fy) + bottom * fy);
        }
    }
    out
}

fn neighbourhood(cell: &[f32], pick: fn(f32, f32) -> f32) -> Vec<f32> {
    let side = SIDE as isize;
    (0..SIDE * SIDE)
        .map(|i| {
            let (x, y) = ((i % SIDE) as isize, (i / SIDE) as isize);
            let mut value = cell[i];
            for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                if (0..side).contains(&nx) && (0..side).contains(&ny) {
                    value = pick(value, cell[(ny * side + nx) as usize]);
                }
            }
            value
        })
        .collect()
}

fn blend(cell: &mut [f32], other: &[f32], weight: f32) {
    cell.iter_mut()
        .zip(other)
        .for_each(|(c, o)| *c += (o - *c) * weight);
}

fn blurred(cell: &[f32]) -> Vec<f32> {
    let side = SIDE as isize;
    (0..SIDE * SIDE)
        .map(|i| {
            let (x, y) = ((i % SIDE) as isize, (i / SIDE) as isize);
            let (mut sum, mut count) = (0.0, 0.0);
            for ny in y - 1..=y + 1 {
                for nx in x - 1..=x + 1 {
                    if (0..side).contains(&nx) && (0..side).contains(&ny) {
                        sum += cell[(ny * side + nx) as usize];
                        count += 1.0;
                    }
                }
            }
            sum / count
        })
        .collect()
}

fn renormalised(mut cell: Vec<f32>) -> Vec<f32> {
    let mean = cell.iter().sum::<f32>() / cell.len() as f32;
    cell.iter_mut().for_each(|v| *v -= mean);
    let length = cell.iter().map(|v| v * v).sum::<f32>().sqrt();
    if length > 0.0 {
        cell.iter_mut().for_each(|v| *v /= length);
    }
    cell
}

pub fn distort(cell: Vec<f32>, random: &mut Random) -> Vec<f32> {
    if random.unit() < CHANCE_CLEAN {
        return cell;
    }
    let mut out = warp(&cell, random);
    if random.unit() < STROKE_CHANCE {
        let pick: fn(f32, f32) -> f32 = if random.unit() < 0.5 {
            f32::max
        } else {
            f32::min
        };
        let mut changed = neighbourhood(&out, pick);
        for _ in 1..=random.bits() % EXTRA_STROKE_STEP_CHOICES {
            changed = neighbourhood(&changed, pick);
        }
        blend(&mut out, &changed, between(random, 0.3, 1.0));
    }
    if random.unit() < BLUR_CHANCE {
        let changed = blurred(&out);
        blend(&mut out, &changed, between(random, 0.3, 1.0));
    }
    let noise = between(random, 0.0, MOST_NOISE) / SIDE as f32;
    out.iter_mut().for_each(|v| *v += random.normal() * noise);
    renormalised(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_distorted_cell_stays_unit_length_and_close_to_its_source() {
        let mut cell = vec![0.0f32; SIDE * SIDE];
        for y in 8..24 {
            cell[y * SIDE + 16] = 1.0;
            cell[16 * SIDE + y] = 1.0;
        }
        let cell = renormalised(blurred(&cell));
        let mut random = Random::new(3);
        for _ in 0..50 {
            let out = distort(cell.clone(), &mut random);
            let length = out.iter().map(|v| v * v).sum::<f32>().sqrt();
            let likeness: f32 = out.iter().zip(&cell).map(|(a, b)| a * b).sum();
            assert!((length - 1.0).abs() < 1e-3, "{length}");
            assert!(likeness > 0.5, "{likeness}");
        }
    }
}
