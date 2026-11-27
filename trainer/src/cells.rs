use std::path::Path;

use crate::list::Picture;
use crate::{CLASSES, INPUT, NOT_GLYPH, each};

const RECORD: usize = 1 + INPUT;
const KEY: usize = 8;
const MOST_DIFFERENT: usize = 2;
const BLOB_LEVELS: [u8; 6] = [235, 210, 185, 160, 135, 110];
const MOST_BLOBS: usize = 1500;

pub struct Cells(Vec<u8>);

impl Cells {
    pub fn len(&self) -> usize {
        self.0.len() / RECORD
    }

    pub fn label(&self, i: usize) -> usize {
        usize::from(self.0[i * RECORD])
    }

    pub fn input(&self, i: usize) -> Vec<f32> {
        let raw = &self.0[i * RECORD + 1..(i + 1) * RECORD];
        let values: Vec<f32> = raw.iter().map(|&v| f32::from(v as i8)).collect();
        let length = values.iter().map(|v| v * v).sum::<f32>().sqrt();
        values.iter().map(|v| v / length).collect()
    }

    pub fn per_class(&self) -> [usize; CLASSES] {
        let mut out = [0; CLASSES];
        (0..self.len()).for_each(|i| out[self.label(i)] += 1);
        out
    }
}

enum Cut {
    Glyphs(Vec<u8>),
    NotGlyphs(Vec<u8>),
    Skipped(String),
}

fn record(out: &mut Vec<u8>, label: u8, cell: &[f32]) {
    let largest = cell.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    out.push(label);
    out.extend(
        cell.iter()
            .map(|v| (v / largest * 127.0).round() as i8 as u8),
    );
}

fn cut(picture: &Picture) -> Cut {
    let bytes = match std::fs::read(&picture.path) {
        Ok(bytes) => bytes,
        Err(error) => return Cut::Skipped(format!("cannot open ({error})")),
    };
    let mut out = Vec::new();
    let Some(glyphs) = &picture.glyphs else {
        let rows = nms_glyph_reader::train::every_row_cells(&bytes);
        let blobs = nms_glyph_reader::train::shape_cells(&bytes, &BLOB_LEVELS);
        let stride = blobs.len().div_ceil(MOST_BLOBS).max(1);
        for cell in rows.iter().chain(blobs.iter().step_by(stride)) {
            record(&mut out, NOT_GLYPH as u8, cell);
        }
        return Cut::NotGlyphs(out);
    };
    let Some((read, rows)) = nms_glyph_reader::train::glyph_rows(&bytes) else {
        return Cut::Skipped("no glyph row found".to_owned());
    };
    let different = read
        .chars()
        .zip(glyphs.chars())
        .filter(|(a, b)| a != b)
        .count();
    if different > MOST_DIFFERENT {
        return Cut::Skipped(format!("the row found reads {read}"));
    }
    for row in rows {
        for (cell, digit) in row.iter().zip(glyphs.chars()) {
            record(&mut out, digit.to_digit(16).unwrap() as u8, cell);
        }
    }
    Cut::Glyphs(out)
}

fn hash(bytes: impl Iterator<Item = u8>) -> [u8; KEY] {
    bytes
        .fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        })
        .to_le_bytes()
}

pub fn load(list: &Path, cache: &Path) -> Cells {
    let key = hash(
        std::fs::read(list)
            .unwrap()
            .into_iter()
            .chain(nms_glyph_reader::train::net().to_bytes()),
    );
    if let Ok(mut bytes) = std::fs::read(cache)
        && bytes.starts_with(&key)
    {
        println!("cells cut before, from {}", cache.display());
        bytes.drain(..KEY);
        return Cells(bytes);
    }
    let pictures = crate::list::read(list);
    let cuts = each(&pictures, cut);
    let mut bytes = key.to_vec();
    let (mut with, mut without, mut skipped) = (0, 0, 0);
    for (picture, cut) in pictures.iter().zip(cuts) {
        match cut {
            Cut::Glyphs(records) => {
                with += 1;
                bytes.extend(records);
            }
            Cut::NotGlyphs(records) => {
                without += 1;
                bytes.extend(records);
            }
            Cut::Skipped(why) => {
                skipped += 1;
                println!("skipped {}: {why}", picture.path.display());
            }
        }
    }
    println!("{with} pictures with glyphs, {without} without, {skipped} skipped");
    std::fs::write(cache, &bytes).unwrap();
    bytes.drain(..KEY);
    Cells(bytes)
}
