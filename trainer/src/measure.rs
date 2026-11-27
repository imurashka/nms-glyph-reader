use std::collections::BTreeMap;
use std::path::Path;

use nms_glyph_reader::Reading;
use nms_glyph_reader::train::use_net;

use crate::each;
use crate::list::{self, Picture};
use crate::net::{Net, Training};

pub fn run(list: &Path, model: Option<&Path>) {
    if let Some(model) = model {
        use_net(Net::load(model));
    }
    let pictures = list::read(list);
    let reads = each(&pictures, |p| {
        std::fs::read(&p.path)
            .ok()
            .and_then(|bytes| nms_glyph_reader::read(&bytes).ok())
    });
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for (picture, read) in pictures.iter().zip(&reads) {
        let Some(read) = read else {
            *counts.entry("cannot open").or_default() += 1;
            println!("cannot open\t{}", picture.path.display());
            continue;
        };
        for (outcome, listed, got) in outcomes(picture, read) {
            *counts.entry(outcome).or_default() += 1;
            if !outcome.ends_with("right") && outcome != "no glyphs, none read" {
                println!("{outcome}\t{}\t{listed}\t{got}", picture.path.display());
            }
        }
    }
    for (outcome, count) in counts {
        println!("{outcome}: {count}");
    }
}

fn outcomes<'a>(picture: &'a Picture, read: &'a Reading) -> Vec<(&'static str, &'a str, &'a str)> {
    let glyphs = read.address.as_deref();
    let listed = picture.glyphs.as_deref();
    let mut out = vec![(
        match (listed, glyphs) {
            (Some(want), Some(got)) if want == got => "glyphs right",
            (Some(_), Some(_)) => "glyphs wrong",
            (Some(_), None) => "glyphs unread",
            (None, Some(_)) => "glyphs read where none are listed",
            (None, None) => "no glyphs, none read",
        },
        listed.unwrap_or("-"),
        glyphs.unwrap_or("-"),
    )];
    if let Some(want) = picture.galaxy.as_deref() {
        let got = read.galaxy.as_deref();
        out.push((
            match got {
                Some(got) if got == want => "galaxy right",
                Some(_) => "galaxy wrong",
                None => "galaxy unread",
            },
            want,
            got.unwrap_or("-"),
        ));
    }
    out
}
