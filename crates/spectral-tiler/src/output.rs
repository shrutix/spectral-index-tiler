//! Writing PNG tiles.

use crate::TilerError;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

/// Write RGBA bytes (4 per pixel, row by row) as an 8-bit PNG.
pub fn write_png_rgba(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), TilerError> {
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;
    writer.finish()?;
    Ok(())
}