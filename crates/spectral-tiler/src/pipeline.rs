//! The end-to-end job: read two bands, compute the index, write the tiles.

use crate::output::write_png_rgba;
use crate::raster::read_band;
use crate::TilerError;
use spectral_core::colormap::{Ramp, Stretch};
use spectral_core::index::{normalized_difference_parallel, Calibration};
use spectral_core::manifest::Manifest;
use spectral_core::stats::{summarize, Stats};
use spectral_core::tiles::{render_tile, tile_grid};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexKind {
    /// Vegetation: (NIR - red) / (NIR + red).
    Ndvi,
    /// Open water (McFeeters): (green - NIR) / (green + NIR).
    Ndwi,
}

impl IndexKind {
    pub fn name(self) -> &'static str {
        match self {
            IndexKind::Ndvi => "ndvi",
            IndexKind::Ndwi => "ndwi",
        }
    }

    pub fn ramp_name(self) -> &'static str {
        match self {
            IndexKind::Ndvi => "vegetation",
            IndexKind::Ndwi => "water",
        }
    }
}