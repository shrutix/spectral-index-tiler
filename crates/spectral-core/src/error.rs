use std::fmt;

/// Errors from the dependency-free core.
#[derive(Debug, Clone, PartialEq)]
pub enum CoreError {
    /// The two input bands do not have the same number of pixels.
    LengthMismatch { first: usize, second: usize },
    /// A tile size of zero was requested.
    InvalidTileSize,
    /// An image with zero width or height.
    EmptyImage,
    /// A color ramp or value stretch that is not usable.
    InvalidRamp(String),
    /// A tile window that does not fit inside the image.
    InvalidWindow(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::LengthMismatch { first, second } => write!(
                f,
                "the two bands have different pixel counts ({first} vs {second})"
            ),
            CoreError::InvalidTileSize => write!(f, "tile size must be greater than zero"),
            CoreError::EmptyImage => write!(f, "the image has no pixels"),
            CoreError::InvalidRamp(message) => write!(f, "invalid color setting: {message}"),
            CoreError::InvalidWindow(message) => write!(f, "invalid tile window: {message}"),
        }
    }
}

impl std::error::Error for CoreError {}
