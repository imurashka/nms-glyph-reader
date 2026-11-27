#![doc = include_str!("README.md")]

mod net;
mod picture;
mod steps;
#[cfg(feature = "train")]
#[doc(hidden)]
pub mod train;

use picture::decode;
use steps::{address, galaxy, glyphs, rows, shapes};

/// What [`read`] found on a picture.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reading {
    /// The portal address: twelve glyphs as twelve hex digits, such as `217EB09D79D7`.
    /// `None` when no glyphs were found, or the reader is not sure enough of them,
    /// or they are not an address the game can open.
    pub address: Option<String>,
    /// The galaxy's English name, such as `Euclid`, read from the line under the glyphs.
    /// `None` when no galaxy name clearly matches.
    pub galaxy: Option<String>,
    /// Where the row of twelve glyphs is in the picture. Given whenever the reader
    /// found the row, also when it is not sure enough of the glyphs to give the address.
    pub glyphs_area: Option<Area>,
    /// Where the galaxy name is in the picture. Given with [`Reading::galaxy`].
    pub galaxy_area: Option<Area>,
}

/// A rectangle in the picture, in pixels from its top left corner, after the
/// picture's EXIF orientation is applied.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Area {
    /// The left edge.
    pub x: u32,
    /// The top edge.
    pub y: u32,
    /// The width.
    pub width: u32,
    /// The height.
    pub height: u32,
}

impl Area {
    fn inside(rect: &picture::Rect, picture: &image::RgbImage) -> Area {
        let clamp = |v: i32, end: u32| v.clamp(0, end as i32) as u32;
        let (left, top) = (
            clamp(rect.left, picture.width()),
            clamp(rect.top, picture.height()),
        );
        let (right, bottom) = (
            clamp(rect.right, picture.width()),
            clamp(rect.bottom, picture.height()),
        );
        Area {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        }
    }
}

/// Why [`read`] did not read a picture.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The bytes are not a JPEG, PNG, GIF or WebP picture, or the picture is broken.
    InvalidImage,
    /// The picture is over 50 megapixels.
    TooLarge,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidImage => "not a readable JPEG, PNG, GIF or WebP picture",
            Self::TooLarge => "over 50 megapixels",
        })
    }
}

impl std::error::Error for Error {}

/// The English names of the 256 galaxies in the game's order: `GALAXIES[0]` is
/// `Euclid`, the galaxy the game numbers 1. [`Reading::galaxy`] is always one of them.
pub const GALAXIES: [&str; 256] = steps::galaxy::NAMES;

/// Reads the portal address and the galaxy name from the bytes of a screenshot file.
///
/// A picture without glyphs is not an error: it gives a [`Reading`] with both
/// fields `None`. See the [crate documentation](crate) for what is read and how
/// long it takes.
///
/// # Errors
///
/// [`Error::InvalidImage`] when the bytes are not a JPEG, PNG, GIF or WebP
/// picture or the picture cannot be decoded, and [`Error::TooLarge`] when it has
/// more than 50 million pixels.
pub fn read(bytes: &[u8]) -> Result<Reading, Error> {
    let picture = decode::picture(bytes)?;
    // Step 1: cut the picture into shapes, at three brightness levels.
    let layers = shapes::cut(&picture, &shapes::LEVELS);
    // Step 2: find candidate rows of twelve shapes in every layer.
    let rows = rows::find(&layers);
    // Step 3: read each glyph, drop rows that are not glyphs, pick the best row.
    // When that row fails the check of step 5, steps 1 and 2 run again inside step 3,
    // for ink lighter or darker than what is around it.
    let Some(glyphs) = glyphs::read(&picture, &layers, &rows) else {
        return Ok(Reading::default());
    };
    // Step 4: read the galaxy from the line under the glyphs.
    let galaxy = galaxy::name(&picture, &glyphs.strip);
    // Step 5: give the address only when it passes the check.
    let address = address::checked(&glyphs);
    Ok(Reading {
        address,
        galaxy: galaxy.map(|(name, _)| name.to_owned()),
        glyphs_area: Some(Area::inside(&glyphs.strip, &picture)),
        galaxy_area: galaxy.map(|(_, area)| Area::inside(&area, &picture)),
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{ImageFormat, RgbImage};

    use super::*;

    fn strip_of(glyphs: &str) -> Vec<u8> {
        let mut picture = RgbImage::from_pixel(800, 300, image::Rgb([24, 26, 30]));
        for (i, d) in glyphs.chars().enumerate() {
            let path = format!("{}/tests/font/font_{d}.png", env!("CARGO_MANIFEST_DIR"));
            let glyph = image::open(&path).unwrap().into_luma8();
            let glyph =
                image::imageops::resize(&glyph, 55, 60, image::imageops::FilterType::Triangle);
            for (x, y, p) in glyph.enumerate_pixels() {
                if p.0[0] > 127 {
                    picture.put_pixel(40 + 60 * i as u32 + x, 100 + y, image::Rgb([235, 240, 238]));
                }
            }
        }
        let mut png = Vec::new();
        picture
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        png
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut png = Vec::new();
        image::GrayImage::new(width, height)
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        png
    }

    #[test]
    fn a_row_of_font_glyphs_reads_as_its_address() {
        let png = strip_of("107407647D3B");
        let picture = decode::picture(&png).unwrap();
        let layers = shapes::cut(&picture, &shapes::LEVELS);
        let glyphs = glyphs::read(&picture, &layers, &rows::find(&layers)).unwrap();
        assert!(glyphs.score > 0.95, "{}", glyphs.score);
        let reading = read(&png).unwrap();
        assert_eq!(reading.address.as_deref(), Some("107407647D3B"));
        assert_eq!(reading.galaxy, None);
        assert_eq!(reading.galaxy_area, None);
        let area = reading.glyphs_area.unwrap();
        let (row_left, row_top, row_right, row_bottom) = (40, 100, 40 + 60 * 11 + 55, 160);
        assert!(area.x >= row_left && area.y >= row_top, "{area:?}");
        assert!(
            area.x + area.width <= row_right && area.y + area.height <= row_bottom,
            "{area:?}"
        );
        assert!(area.width >= 9 * (row_right - row_left) / 10, "{area:?}");
    }

    #[test]
    fn what_is_not_a_picture_is_an_error() {
        assert_eq!(read(b"\x89PNG"), Err(Error::InvalidImage));
        assert_eq!(read(b""), Err(Error::InvalidImage));
        assert_eq!(read(&png(10_000, 6_000)), Err(Error::TooLarge));
    }

    #[test]
    fn a_picture_without_glyphs_reads_nothing() {
        assert_eq!(read(&png(8000, 2)), Ok(Reading::default()));
        assert_eq!(read(&png(640, 480)), Ok(Reading::default()));
    }
}
