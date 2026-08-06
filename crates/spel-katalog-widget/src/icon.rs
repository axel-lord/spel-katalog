//! [Icon] impl.

use ::derive_more::IsVariant;
use ::iced_core::{Color, Element, Theme};
use ::iced_widget::{Button, Renderer, button};
use ::spel_katalog_assets as assets;

/// Status of icon.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, IsVariant)]
pub enum Status {
    /// The icon is enabled.
    #[default]
    Enabled,
    /// The icon is disabled.
    Disabled,
}

/// Display an icon.
#[derive(Debug, Clone)]
pub struct Icon<'a> {
    /// Handle to svg.
    pub handle: &'a ::iced_core::svg::Handle,
    /// Icon size.
    pub size: u32,
    /// Icon style.
    pub style: Option<fn(&Theme, Status) -> Color>,
    /// Is the icon enabled.
    pub status: Status,
}

pub mod outline {
    //! Outline style functions.

    use ::iced_core::{Color, Theme};
    use ::iced_widget::button::Status;
    use ::spel_katalog_color::{HsluvaExt, palette::Desaturate};

    /// Success outline.
    pub fn success(theme: &Theme, status: Status) -> Color {
        match status {
            Status::Active => theme.extended_palette().success.base.color,
            Status::Hovered => theme.extended_palette().success.strong.color,
            Status::Pressed => theme.extended_palette().success.base.color,
            Status::Disabled => theme
                .extended_palette()
                .success
                .weak
                .color
                .with_hsluva(|color| color.desaturate(0.25)),
        }
    }

    /// Primary outline.
    pub fn primary(theme: &Theme, status: Status) -> Color {
        match status {
            Status::Active => theme.extended_palette().primary.base.color,
            Status::Hovered => theme.extended_palette().primary.strong.color,
            Status::Pressed => theme.extended_palette().primary.base.color,
            Status::Disabled => theme
                .extended_palette()
                .primary
                .weak
                .color
                .with_hsluva(|color| color.desaturate(0.25)),
        }
    }

    /// Danger outline.
    pub fn danger(theme: &Theme, status: Status) -> Color {
        match status {
            Status::Active => theme.extended_palette().danger.base.color,
            Status::Hovered => theme.extended_palette().danger.strong.color,
            Status::Pressed => theme.extended_palette().danger.base.color,
            Status::Disabled => theme
                .extended_palette()
                .danger
                .weak
                .color
                .with_hsluva(|color| color.desaturate(0.25)),
        }
    }
}

/// Get a close button.
pub fn close<'a, M: 'a>() -> Button<'a, M, Theme, Renderer> {
    Icon::new(assets::cross())
        .size(20)
        .into_button()
        .style(button::danger)
}

/// Get a minimize button.
pub fn minimize<'a, M: 'a>() -> Button<'a, M, Theme, Renderer> {
    Icon::new(assets::minimize())
        .size(20)
        .into_button()
        .style(button::danger)
}

impl<'a, M: 'a> From<Icon<'a>> for Element<'a, M, Theme, Renderer> {
    fn from(value: Icon<'a>) -> Self {
        value.into_svg().into()
    }
}

impl<'a> Icon<'a> {
    /// Create a new icon.
    pub const fn new(handle: &'a ::iced_core::svg::Handle) -> Self {
        Self {
            handle,
            size: 26,
            style: None,
            status: Status::Enabled,
        }
    }

    /// Set icon style.
    pub const fn style(mut self, style: fn(&Theme, Status) -> Color) -> Self {
        self.style = Some(style);
        self
    }

    /// Set icon size.
    pub const fn size(self, size: u32) -> Self {
        Self { size, ..self }
    }

    /// Set status.
    pub const fn status(mut self, status: Status) -> Self {
        self.status = status;
        self
    }

    /// Set status to enabled if condition is true else disabled.
    pub const fn enabled(mut self, cond: bool) -> Self {
        self.status = if cond {
            Status::Enabled
        } else {
            Status::Disabled
        };
        self
    }

    /// Create a simple icon.
    pub fn into_svg(self) -> ::iced_widget::svg::Svg<'a, Theme> {
        let Icon {
            handle,
            size,
            style,
            status,
        } = self;
        let svg = ::iced_widget::Svg::new(handle.clone())
            .width(size)
            .height(size);
        if let Some(style) = style {
            svg.style(move |theme: &Theme, _| ::iced_widget::svg::Style {
                color: Some(style(theme, status)),
            })
        } else {
            svg.style(move |theme: &Theme, _| ::iced_widget::svg::Style {
                color: Some(match status {
                    Status::Enabled => theme.extended_palette().background.strongest.text,
                    Status::Disabled => theme.extended_palette().background.weakest.text,
                }),
            })
        }
    }

    /// Create a button from icon.
    pub fn into_button<M: 'a>(self) -> Button<'a, M, Theme, Renderer> {
        button(self.into_svg()).padding(3)
    }

    /// Crate a button from icon.
    pub fn into_outline_button<M: 'a>(
        self,
        style: impl Fn(&Theme, button::Status) -> Color + 'a,
    ) -> Button<'a, M, Theme, Renderer> {
        button(self.into_svg())
            .style(move |theme, status| {
                let mut base = button::background(theme, status);
                base.border.width = 1.5;
                base.border.color = style(theme, status);
                base
            })
            .padding(5)
    }
}
