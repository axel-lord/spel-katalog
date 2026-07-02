//! [Image] impl.
use ::std::borrow::Cow;

pub use ::bytes::Bytes;
use ::image::{DynamicImage, RgbaImage};
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

impl Image {
    /// Convert to an rgba image.
    #[inline]
    pub fn to_rgba(&self) -> Option<RgbaImage> {
        let Self {
            width,
            height,
            bytes,
        } = self;
        RgbaImage::from_raw(*width, *height, bytes.to_vec())
    }

    /// Convert into an rgba image.
    #[inline]
    pub fn into_rgba(self) -> Option<RgbaImage> {
        let Self {
            width,
            height,
            bytes,
        } = self;
        RgbaImage::from_raw(width, height, Vec::<u8>::from(bytes))
    }

    /// Convert to a dynamic image.
    #[inline]
    pub fn to_image(&self) -> Option<DynamicImage> {
        Some(self.to_rgba()?.into())
    }

    /// Convert into a dynamic image.
    #[inline]
    pub fn into_image(self) -> Option<DynamicImage> {
        Some(self.into_rgba()?.into())
    }

    /// Convert and rgba image to self.
    #[inline]
    pub fn from_rgba(image: RgbaImage) -> Self {
        let width = image.width();
        let height = image.height();
        let bytes = Bytes::from_owner(image.into_raw());
        Self {
            width,
            height,
            bytes,
        }
    }

    /// Conver an image to self.
    #[inline]
    pub fn from_image(image: DynamicImage) -> Self {
        Self::from_rgba(image.into_rgba8())
    }

    /// Give a function width, height and bytes to create a value of some type.
    #[inline]
    pub fn map<F, T>(self, f: F) -> T
    where
        F: FnOnce(u32, u32, Bytes) -> T,
    {
        let Self {
            width,
            height,
            bytes,
        } = self;
        f(width, height, bytes)
    }
}

impl From<RgbaImage> for Image {
    #[inline]
    fn from(value: RgbaImage) -> Self {
        Image::from_rgba(value)
    }
}

impl From<DynamicImage> for Image {
    #[inline]
    fn from(value: DynamicImage) -> Self {
        Image::from_image(value)
    }
}

impl From<Cow<'_, DynamicImage>> for Image {
    #[inline]
    fn from(value: Cow<'_, DynamicImage>) -> Self {
        match value {
            Cow::Borrowed(i) => Image::from_rgba(i.to_rgba8()),
            Cow::Owned(i) => Image::from_image(i),
        }
    }
}
