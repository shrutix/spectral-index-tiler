//! Reading a single-band GeoTIFF into memory.
//!
//! This first version reads the whole band at once, so a full Sentinel-2 tile
//! band (about 120 million pixels, 240 MB) is the practical upper limit.
//! Clip large scenes first; windowed reading is the first planned extension.

use crate::TilerError;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use tiff::decoder::{Decoder, DecodingResult};
use tiff::ColorType;

/// One band as unsigned 16-bit values, row by row.
pub struct Band {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u16>,
}

pub fn read_band(path: &Path) -> Result<Band, TilerError> {
    let file = File::open(path).map_err(|source| TilerError::Open {
        path: path.to_path_buf(),
        source,
    })?;
    let mut decoder = Decoder::new(BufReader::new(file))?;
    let (width, height) = decoder.dimensions()?;

    match decoder.colortype()? {
        ColorType::Gray(_) => {}
        other => {
            return Err(TilerError::Unsupported(format!(
                "{} must be a single-band image, found {other:?}",
                path.display()
            )))
        }
    }

    let data = match decoder.read_image()? {
        DecodingResult::U16(values) => values,
        DecodingResult::U8(values) => values.into_iter().map(u16::from).collect(),
        _ => {
            return Err(TilerError::Unsupported(format!(
                "{} must hold 8-bit or 16-bit unsigned integers",
                path.display()
            )))
        }
    };

    if data.len() != width as usize * height as usize {
        return Err(TilerError::Unsupported(format!(
            "{} decoded to an unexpected number of pixels",
            path.display()
        )));
    }
    Ok(Band {
        width,
        height,
        data,
    })
}
