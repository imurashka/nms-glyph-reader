Reads the portal address and the galaxy from a No Man's Sky screenshot.

`read` takes the bytes of a picture file and returns a `Reading`: the
portal address as twelve hex digits, such as `217EB09D79D7`, and the
galaxy's English name, such as `Euclid`. Each is `None` when the reader is
not sure of it, so a value it gives is meant to be right, and a value it is
unsure of is left out.

```rust,no_run
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read("portal.jpg")?;
    let reading = nms_glyph_reader::read(&bytes)?;
    match (reading.address, reading.galaxy) {
        (Some(address), Some(galaxy)) => println!("{address} in {galaxy}"),
        (Some(address), None) => println!("{address}, galaxy unknown"),
        (None, _) => println!("no address"),
    }
    Ok(())
}
```

# What it reads

- **The address**: twelve portal glyphs in a row, wherever they are on the
  screen. A small neural net reads each glyph. The address is given only when
  the net is sure of every glyph and the game can open the address: a planet
  from 0 to 6, a system that exists, coordinates that exist.
- **The galaxy**: the line the game's photo mode prints under the glyphs. All
  256 names are recognised in English, and the five named galaxies (Euclid,
  Hilbert Dimension, Calypso, Hesperius Dimension, Hyades) also in nine other
  languages. The answer is always the English name.
- **Where they are**: `glyphs_area` and `galaxy_area` give the rectangle of the
  glyph row and of the galaxy name in the picture, to show or crop them.
- **Pictures**: JPEG, PNG, GIF (the first frame) and WebP, up to 50 megapixels.
  EXIF orientation is applied. A picture under 200 pixels both ways reads
  nothing.

# Accuracy

Measured on screenshots from Reddit posts:

| Posts | Screenshots | Address right | Address wrong | No address |
|---|---:|---:|---:|---:|
| 2025-2026 | 17,576 | 17,510 | 14 | 52 |
| 2017-2024 | 2,000 | 1,990 | 2 | 8 |

Of the 9,727 screenshots that show the photo mode's galaxy line, the galaxy
was given on 9,696 and was never wrong.

# Performance

A 1920 x 1080 screenshot takes about 30 ms on one core of a desktop CPU in a
release build, and well under a second in a debug build. Two calls take
longer than the rest: the first one that finds glyphs, which draws the galaxy
names once, and one on a picture without an address the reader accepts, which
searches the picture again for glyphs lighter or darker than what is around
them. `read` keeps no state between calls besides those drawn names (about
5 MB), and can be called from several threads at once.

# Building

The crate is pure Rust. The minimum Rust version is 1.88.

# Command line

The crate is also an app: `cargo install nms-glyph-reader`, then
`nms-glyph-reader screenshot.jpg` prints `217EB09D79D7 01D6:002F:01D6:017E Euclid`:
the address, the galactic coordinates and the galaxy. `-f json` and `-f csv` print
the same as JSON Lines or CSV. Ready builds
for Windows, macOS and Linux are on the
[releases page](https://github.com/imurashka/nms-glyph-reader/releases).

How the reader works and how its net is trained: the
[README](https://github.com/imurashka/nms-glyph-reader#readme).
