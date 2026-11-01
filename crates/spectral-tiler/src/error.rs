use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TilerError {
    #[error("cannot open {}: {source}", path.display())]
    Open {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{0}")]
    Unsupported(String),

    #[error("the two bands differ in size: {first_width}x{first_height} vs {second_width}x{second_height}")]
    SizeMismatch {
        first_width: u32,
        first_height: u32,
        second_width: u32,
        second_height: u32,
    },

    #[error("invalid option: {0}")]
    Invalid(String),

    #[error(transparent)]
    Core(#[from] spectral_core::CoreError),

    #[error("TIFF error: {0}")]
    Tiff(#[from] tiff::TiffError),

    #[error("PNG error: {0}")]
    Png(#[from] png::EncodingError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
