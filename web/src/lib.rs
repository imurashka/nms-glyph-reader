use std::sync::Mutex;

use nms_glyph_reader::{Area, GALAXIES};

static ANSWER: Mutex<String> = Mutex::new(String::new());

/// Reserves `len` bytes in the module's memory for the page to copy a picture file into,
/// and returns where they start. [`read_picture`] takes them back.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut bytes = Vec::<u8>::with_capacity(len);
    let pointer = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    pointer
}

/// Reads the picture file at `pointer` and keeps the reading as one line of JSON, which
/// [`answer`] points at; returns the line's length in bytes.
///
/// # Safety
///
/// `pointer` and `len` must come from one call of [`alloc`] whose bytes the page has
/// filled, and are not used again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn read_picture(pointer: *mut u8, len: usize) -> usize {
    let bytes = unsafe { Vec::from_raw_parts(pointer, len, len) };
    let mut stored = ANSWER.lock().unwrap();
    *stored = match nms_glyph_reader::read(&bytes) {
        Ok(reading) => format!(
            "{{\"address\":{},\"galaxy\":{},\"galaxy_number\":{},\"glyphs_area\":{},\"galaxy_area\":{}}}",
            quoted(reading.address.as_deref()),
            quoted(reading.galaxy.as_deref()),
            reading
                .galaxy
                .as_deref()
                .and_then(|galaxy| GALAXIES.iter().position(|name| *name == galaxy))
                .map_or("null".to_owned(), |index| (index + 1).to_string()),
            box_of(reading.glyphs_area),
            box_of(reading.galaxy_area)
        ),
        Err(error) => format!("{{\"error\":\"{error}\"}}"),
    };
    stored.len()
}

/// Where the line of JSON the last [`read_picture`] kept starts.
#[unsafe(no_mangle)]
pub extern "C" fn answer() -> *const u8 {
    ANSWER.lock().unwrap().as_ptr()
}

fn quoted(value: Option<&str>) -> String {
    value.map_or("null".to_owned(), |text| format!("\"{text}\""))
}

fn box_of(area: Option<Area>) -> String {
    area.map_or("null".to_owned(), |a| {
        format!("[{},{},{},{}]", a.x, a.y, a.width, a.height)
    })
}
