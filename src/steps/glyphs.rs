use std::cmp::Ordering;
use std::collections::HashMap;

use image::RgbImage;

use super::address;
use super::rows::{self, GLYPHS, Row};
use super::shapes::{self, Layer};
use crate::net::{self, cell};
use crate::picture::{Mask, Rect};

const SURELY_NOT_A_GLYPH: f32 = 0.99;

pub struct Glyphs {
    pub text: String,
    pub score: f64,
    pub strip: Rect,
    #[cfg_attr(not(feature = "train"), expect(dead_code))]
    pub voters: Vec<Vec<Mask>>,
}

struct Candidate<'a> {
    layer: &'a Layer,
    row: &'a Row,
    probabilities: Vec<Vec<f32>>,
}

impl Candidate<'_> {
    fn confidence(&self) -> f32 {
        self.probabilities.iter().map(|p| p[likeliest(p)]).sum()
    }

    fn lies_on(&self, other: &Candidate) -> bool {
        let mut cells = self.row.cells.iter().zip(&other.row.cells);
        cells.all(|(cell, other)| {
            (2 * other.left..=2 * other.right).contains(&(cell.left + cell.right))
                && (2 * other.top..=2 * other.bottom).contains(&(cell.top + cell.bottom))
        })
    }
}

fn by_confidence(a: &&Candidate, b: &&Candidate) -> Ordering {
    a.confidence().total_cmp(&b.confidence())
}

fn likeliest(probabilities: &[f32]) -> usize {
    (0..net::NOT_GLYPH)
        .max_by(|&a, &b| probabilities[a].total_cmp(&probabilities[b]))
        .unwrap()
}

pub fn read(picture: &RgbImage, layers: &[Layer], rows: &[Vec<Row>]) -> Option<Glyphs> {
    let mut candidates = read_rows(layers, rows);
    let glyphs = voted(&candidates);
    if glyphs.as_ref().is_some_and(address::accepted) {
        return glyphs;
    }
    let local = shapes::cut(picture, &shapes::LOCAL);
    let local_rows = rows::find(&local);
    candidates.extend(read_rows(&local, &local_rows));
    better(glyphs, voted(&candidates))
}

fn read_rows<'a>(layers: &'a [Layer], rows: &'a [Vec<Row>]) -> Vec<Candidate<'a>> {
    let mut candidates = Vec::new();
    for (layer, rows) in layers.iter().zip(rows) {
        let mut read: HashMap<Rect, Option<Vec<f32>>> = HashMap::new();
        for row in rows {
            let probabilities: Option<Vec<Vec<f32>>> = row
                .cells
                .iter()
                .map(|cell| {
                    read.entry(*cell)
                        .or_insert_with(|| cell::probabilities(&layer.mask.crop(cell)))
                        .clone()
                        .filter(|p| p[net::NOT_GLYPH] < SURELY_NOT_A_GLYPH)
                })
                .collect();
            if let Some(probabilities) = probabilities {
                candidates.push(Candidate {
                    layer,
                    row,
                    probabilities,
                });
            }
        }
    }
    candidates
}

fn voted(candidates: &[Candidate]) -> Option<Glyphs> {
    let mut scales: Vec<f64> = candidates.iter().map(|c| c.layer.scale).collect();
    scales.sort_by(f64::total_cmp);
    scales.dedup();
    let mut best = None;
    for scale in scales {
        let at_scale: Vec<&Candidate> = candidates
            .iter()
            .filter(|c| c.layer.scale == scale)
            .collect();
        best = better(best, vote(&at_scale));
    }
    best
}

fn better(best: Option<Glyphs>, next: Option<Glyphs>) -> Option<Glyphs> {
    let rank = |g: &Glyphs| g.score + f64::from(u8::from(address::plausible(&g.text)));
    match (best, next) {
        (Some(best), Some(next)) if rank(&next) < rank(&best) => Some(best),
        (best, None) => best,
        (_, next) => next,
    }
}

fn vote(candidates: &[&Candidate]) -> Option<Glyphs> {
    let best = candidates.iter().copied().max_by(by_confidence)?;
    let voters: Vec<&Candidate> = shapes::ALL
        .iter()
        .filter_map(|&ink| {
            candidates
                .iter()
                .copied()
                .filter(|c| c.layer.ink == ink && c.lies_on(best))
                .filter(|c| c.row.whole || std::ptr::eq(*c, best))
                .max_by(by_confidence)
        })
        .collect();
    let votes: Vec<Vec<f32>> = (0..GLYPHS)
        .map(|glyph| {
            (0..=net::NOT_GLYPH)
                .map(|class| {
                    let sum: f32 = voters.iter().map(|v| v.probabilities[glyph][class]).sum();
                    sum / voters.len() as f32
                })
                .collect()
        })
        .collect();
    let cells = best.row.cells.iter().copied();
    let strip = cells.reduce(|strip, cell| strip.joined(&cell)).unwrap();
    let in_picture = |v: i32| (f64::from(v) * best.layer.scale).round() as i32;
    Some(Glyphs {
        text: votes
            .iter()
            .map(|p| cell::DIGITS[likeliest(p)] as char)
            .collect(),
        score: votes
            .iter()
            .map(|p| f64::from(p[likeliest(p)]))
            .fold(1.0, f64::min),
        strip: Rect {
            left: in_picture(strip.left),
            top: in_picture(strip.top),
            right: in_picture(strip.right),
            bottom: in_picture(strip.bottom),
        },
        voters: voters
            .iter()
            .map(|v| v.row.cells.iter().map(|c| v.layer.mask.crop(c)).collect())
            .collect(),
    })
}
