//! Dependency-free core for the spectral index tiler.
//!
//! Everything here is plain Rust with no third-party crates:
//!
//! * [`index`]: normalized-difference indices (NDVI, NDWI) over raw band values,
//!   sequential and multi-threaded
//! * [`colormap`]: color ramps and value stretches that turn index values into RGBA
//! * [`tiles`]: splitting an image into tiles and rendering each one
//! * [`stats`]: summary statistics that ignore invalid pixels
//! * [`manifest`]: a small JSON description of a tiling run
//!
//! File reading, PNG writing, and the command line live in the `spectral-tiler`
//! crate, which depends on this one.

pub mod colormap;
mod error;
pub mod index;
pub mod manifest;
pub mod stats;
pub mod tiles;

pub use error::CoreError;
