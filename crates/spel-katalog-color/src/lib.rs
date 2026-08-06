//! Color manipulation tools.

use ::palette::{Hsluva, IntoColor, Srgba};

/// Re-export of palette.
pub use ::palette;

/// Trait for types which may be converted to and from hsla.
pub trait HsluvaConv {
    /// Convert into hsla.
    fn into_hsluva(self) -> Hsluva;

    /// Convert from hsla.
    fn from_hsluva(color: Hsluva) -> Self;
}

/// Extensions to [HsluvaConv] implementors.
pub trait HsluvaExt: HsluvaConv {
    /// Run function with self as hsla.
    fn with_hsluva<F>(self, f: F) -> Self
    where
        Self: Sized,
        F: FnOnce(Hsluva) -> Hsluva,
    {
        Self::from_hsluva(f(self.into_hsluva()))
    }
}

impl<T> HsluvaExt for T where T: HsluvaConv {}

impl HsluvaConv for Hsluva {
    fn into_hsluva(self) -> Hsluva {
        self
    }

    fn from_hsluva(color: Hsluva) -> Self {
        color
    }
}

impl HsluvaConv for Srgba<f32> {
    fn into_hsluva(self) -> Hsluva {
        self.into_color()
    }

    fn from_hsluva(color: Hsluva) -> Self {
        color.into_color()
    }
}

impl HsluvaConv for Srgba<u8> {
    fn into_hsluva(self) -> Hsluva {
        let color: Srgba<f32> = self.into_format();
        color.into_color()
    }

    fn from_hsluva(color: Hsluva) -> Self {
        let color: Srgba<f32> = color.into_color();
        color.into_format()
    }
}

impl HsluvaConv for [f32; 4] {
    fn into_hsluva(self) -> Hsluva {
        Srgba::from(self).into_color()
    }

    fn from_hsluva(color: Hsluva) -> Self {
        Srgba::<f32>::from_hsluva(color).into()
    }
}

impl HsluvaConv for [u8; 4] {
    fn into_hsluva(self) -> Hsluva {
        Srgba::<u8>::from(self).into_hsluva()
    }

    fn from_hsluva(color: Hsluva) -> Self {
        Srgba::<u8>::from_hsluva(color).into()
    }
}

impl HsluvaConv for ::iced_core::Color {
    fn into_hsluva(self) -> Hsluva {
        let Self { r, g, b, a } = self;
        Srgba::<f32>::new(r, g, b, a).into_hsluva()
    }

    fn from_hsluva(color: Hsluva) -> Self {
        let [r, g, b, a] = Srgba::<f32>::from_hsluva(color).into();
        Self { r, g, b, a }
    }
}
