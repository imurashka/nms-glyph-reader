use std::collections::HashSet;

use crate::net::cell::normalise;
use crate::picture::decode;
use crate::steps::rows;
use crate::steps::shapes::{self, Ink};

pub use crate::net::cell::SIDE;
pub use crate::net::{Dense, NOT_GLYPH, Net, net, softmax, use_net};
pub use crate::steps::rows::GLYPHS;

pub fn glyph_rows(bytes: &[u8]) -> Option<(String, Vec<Vec<Vec<f32>>>)> {
    let picture = decode::picture(bytes).ok()?;
    let layers = shapes::cut(&picture, &shapes::LEVELS);
    let glyphs = crate::steps::glyphs::read(&picture, &layers, &rows::find(&layers))?;
    let rows = glyphs
        .voters
        .iter()
        .filter_map(|cells| cells.iter().map(normalise).collect())
        .collect();
    Some((glyphs.text, rows))
}

pub fn every_row_cells(bytes: &[u8]) -> Vec<Vec<f32>> {
    let Ok(picture) = decode::picture(bytes) else {
        return Vec::new();
    };
    let layers = shapes::cut(&picture, &shapes::ALL);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (layer, rows) in layers.iter().zip(rows::find(&layers)) {
        for cell in rows.iter().flat_map(|row| &row.cells) {
            if seen.insert((layer.scale.to_bits(), layer.ink, *cell)) {
                out.extend(normalise(&layer.mask.crop(cell)));
            }
        }
    }
    out
}

pub fn shape_cells(bytes: &[u8], levels: &[u8]) -> Vec<Vec<f32>> {
    let Ok(picture) = decode::picture(bytes) else {
        return Vec::new();
    };
    let picture = decode::shrunk(&picture).unwrap_or(picture);
    let darkest = shapes::darkest_channel(&picture);
    let mut out = Vec::new();
    for &level in levels {
        let mask = shapes::mask(&picture, &darkest, Ink::Above(level));
        let shapes = shapes::find(&mask, shapes::MOST_SHAPES).unwrap_or_default();
        for shape in shapes.iter().filter(|shape| rows::glyph_sized(shape)) {
            out.extend(normalise(&mask.crop(shape)));
        }
    }
    out
}
