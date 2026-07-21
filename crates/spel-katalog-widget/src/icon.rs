//! [Icon] impl.

use ::iced_core::{Color, Element, Theme};
use ::iced_widget::{Button, Renderer, button};
use ::spel_katalog_assets as assets;

/// Display an icon.
#[derive(Debug, Clone)]
pub struct Icon<'a> {
    /// Handle to svg.
    pub handle: &'a ::iced_core::svg::Handle,
    /// Icon size.
    pub size: u32,
    /// Icon style.
    pub style: Option<fn(&Theme) -> Color>,
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
        let Icon {
            handle,
            size,
            style,
        } = value;
        let svg = ::iced_widget::Svg::new(handle.clone())
            .width(size)
            .height(size);
        if let Some(style) = style {
            svg.style(move |theme: &Theme, _| ::iced_widget::svg::Style {
                color: Some(style(theme)),
            })
        } else {
            svg
        }
        .into()
    }
}

impl<'a> Icon<'a> {
    /// Create a new icon.
    pub fn new(handle: &'a ::iced_core::svg::Handle) -> Self {
        Self {
            handle,
            size: 24,
            style: None,
        }
    }

    /// Set icon style.
    pub fn style(self, style: fn(&Theme) -> Color) -> Self {
        Self {
            style: Some(style),
            ..self
        }
    }

    /// Set icon size.
    pub const fn size(self, size: u32) -> Self {
        Self { size, ..self }
    }

    /// Create a button from icon.
    pub fn into_button<M: 'a>(self) -> Button<'a, M, Theme, Renderer> {
        button(Element::from(self)).padding(3)
    }

    /// Create a button from icon.
    pub fn into_button_with_outline<M: 'a>(
        self,
        outline: fn(&Theme) -> Color,
    ) -> Button<'a, M, Theme, Renderer> {
        button(Element::from(self))
            .style(move |theme, status| {
                let mut base = button::background(theme, status);
                base.border.width = 1.5;
                base.border.color = outline(theme);
                base
            })
            .padding(5)
    }
}
