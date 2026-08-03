//! Color manipulation tools.

use ::palette::{IntoColor, Srgba};

/// Trait for types which may be converted to and from hsla.
pub trait HslaConv {
    /// Convert into hsla.
    fn into_hsla(self) -> ::palette::Hsla;

    /// Convert from hsla.
    fn from_hsla(color: ::palette::Hsla) -> Self;
}

impl HslaConv for Srgba<f32> {
    fn into_hsla(self) -> palette::Hsla {
        self.into_color()
    }

    fn from_hsla(color: palette::Hsla) -> Self {
        color.into_color()
    }
}

impl HslaConv for Srgba<u8> {
    fn into_hsla(self) -> palette::Hsla {
        let color: Srgba<f32> = self.into_format();
        color.into_color()
    }

    fn from_hsla(color: palette::Hsla) -> Self {
        let color: Srgba<f32> = color.into_color();
        color.into_format()
    }
}

impl HslaConv for [f32; 4] {
    fn into_hsla(self) -> palette::Hsla {
        Srgba::from(self).into_color()
    }

    fn from_hsla(color: palette::Hsla) -> Self {
        Srgba::<f32>::from_hsla(color).into()
    }
}

impl HslaConv for [u8; 4] {
    fn into_hsla(self) -> palette::Hsla {
        Srgba::<u8>::from(self).into_hsla()
    }

    fn from_hsla(color: palette::Hsla) -> Self {
        Srgba::<u8>::from_hsla(color).into()
    }
}

impl HslaConv for ::iced_core::Color {
    fn into_hsla(self) -> palette::Hsla {
        let Self { r, g, b, a } = self;
        Srgba::<f32>::new(r, g, b, a).into_hsla()
    }

    fn from_hsla(color: palette::Hsla) -> Self {
        let [r, g, b, a] = Srgba::<f32>::from_hsla(color).into();
        Self { r, g, b, a }
    }
}
