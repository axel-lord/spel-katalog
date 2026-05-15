//! [Image] impl.
pub use ::bytes::Bytes;
pub use ::serde::{Deserialize, Serialize};

/// Bytes and dimensions of an rgba image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    /// Width of image.
    pub width: u32,
    /// Height of image.
    pub height: u32,
    /// Content of image.
    pub bytes: Bytes,
}
