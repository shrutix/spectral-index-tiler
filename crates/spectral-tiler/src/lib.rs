//! Reads two single-band GeoTIFFs, computes a normalized-difference index with
//! `spectral-core`, and writes the result as colored PNG tiles.

mod error;
pub mod output;
pub mod pipeline;
pub mod raster;

pub use error::TilerError;