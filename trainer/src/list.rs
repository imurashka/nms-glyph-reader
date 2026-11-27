use std::path::{Path, PathBuf};

use nms_glyph_reader::train::GLYPHS;

pub struct Picture {
    pub path: PathBuf,
    pub glyphs: Option<String>,
    pub galaxy: Option<String>,
}

pub fn read(list: &Path) -> Vec<Picture> {
    let text =
        std::fs::read_to_string(list).unwrap_or_else(|error| panic!("{}: {error}", list.display()));
    let folder = list.parent().unwrap_or(Path::new(""));
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|(n, line)| {
            parse(line, folder).unwrap_or_else(|| {
                panic!(
                    "{}, line {}: expected <picture>\\t<twelve glyphs or ->\\t<galaxy or ->",
                    list.display(),
                    n + 1
                )
            })
        })
        .collect()
}

fn parse(line: &str, folder: &Path) -> Option<Picture> {
    let mut fields = line.split('\t').map(str::trim);
    let path = folder.join(fields.next()?);
    let glyphs = given(fields.next()?);
    let galaxy = fields.next().and_then(given);
    if let Some(glyphs) = &glyphs
        && (glyphs.len() != GLYPHS || !glyphs.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return None;
    }
    Some(Picture {
        path,
        glyphs: glyphs.map(|g| g.to_ascii_uppercase()),
        galaxy,
    })
}

fn given(field: &str) -> Option<String> {
    (!field.is_empty() && field != "-").then(|| field.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_a_picture_its_glyphs_and_its_galaxy() {
        let folder = Path::new("pictures");
        let picture = parse("a.png\t10a3f9556c30\tHilbert Dimension", folder).unwrap();
        assert_eq!(picture.path, folder.join("a.png"));
        assert_eq!(picture.glyphs.as_deref(), Some("10A3F9556C30"));
        assert_eq!(picture.galaxy.as_deref(), Some("Hilbert Dimension"));
        let none = parse("b.jpg\t-\t-", folder).unwrap();
        assert!(none.glyphs.is_none() && none.galaxy.is_none());
        assert!(parse("c.jpg\t-", folder).unwrap().galaxy.is_none());
        assert!(parse("d.jpg", folder).is_none());
        assert!(parse("e.jpg\t10A3F9556C3\t-", folder).is_none());
        assert!(parse("f.jpg\t10A3F9556C3X\t-", folder).is_none());
    }
}
