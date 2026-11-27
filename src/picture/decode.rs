use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageReader, Limits, RgbImage};

use super::resize;
use crate::Error;

const WIDEST_VIEW: u32 = 1920;
const MOST_PIXELS: u64 = 50_000_000;

pub fn picture(bytes: &[u8]) -> Result<RgbImage, Error> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .or(Err(Error::InvalidImage))?;
    reader.limits(Limits::default());
    let mut decoder = reader.into_decoder().or(Err(Error::InvalidImage))?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 {
        return Err(Error::InvalidImage);
    }
    if u64::from(width) * u64::from(height) > MOST_PIXELS {
        return Err(Error::TooLarge);
    }
    let orientation = decoder.orientation().or(Err(Error::InvalidImage))?;
    let mut picture = DynamicImage::from_decoder(decoder).or(Err(Error::InvalidImage))?;
    picture.apply_orientation(orientation);
    Ok(picture.into_rgb8())
}

pub fn shrunk(picture: &RgbImage) -> Option<RgbImage> {
    let (width, height) = picture.dimensions();
    if width <= WIDEST_VIEW {
        return None;
    }
    let to_height = f64::from(height) * f64::from(WIDEST_VIEW) / f64::from(width);
    let to_height = to_height.round_ties_even() as u32;
    if to_height == 0 {
        return None;
    }
    let pixels = resize::shrink(
        picture.as_raw(),
        (width as usize, height as usize),
        (WIDEST_VIEW as usize, to_height as usize),
    );
    RgbImage::from_raw(WIDEST_VIEW, to_height, pixels)
}
