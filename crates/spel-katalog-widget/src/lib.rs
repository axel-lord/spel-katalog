//! Widgets with application defaults.

use ::iced_core::{Color, Font, Shadow, Vector, text::IntoFragment};
use ::iced_widget::{self as widget, text::Rich};

pub use self::{
    list_menu::{ListMenu, hover_background_text_button, menu_button},
    scrollable::{x_scrollable, xy_end_scrollable, xy_scrollable, y_end_scrollable, y_scrollable},
};

pub mod button;
pub mod icon;
pub mod rule;

mod list_menu;
mod scrollable;

pub use scrollable::y_scrollable as scrollable;

/// Alias to element with defaults for renderer and theme.
pub type Element<'a, M, Theme = ::iced_core::Theme, Renderer = ::iced_widget::Renderer> =
    ::iced_core::Element<'a, M, Theme, Renderer>;

/// Display monospace text.
pub fn monospace<'a, Theme, Renderer>(
    text: impl IntoFragment<'a>,
) -> widget::Text<'a, Theme, Renderer>
where
    Theme: 'a + widget::text::Catalog,
    Renderer: ::iced_core::text::Renderer,
    <Renderer as ::iced_core::text::Renderer>::Font: From<::iced_core::Font>,
{
    widget::text(text).font(Font::MONOSPACE)
}

/// Create an svg icon widget from a handle reference.
pub fn svg_icon(handle: &::iced_core::svg::Handle) -> ::iced_widget::Svg<'_> {
    const DIM: u32 = 24;
    ::iced_widget::Svg::new(handle.clone())
        .width(DIM)
        .height(DIM)
        .style(|theme: &::iced_core::Theme, _| ::iced_widget::svg::Style {
            color: Some(theme.extended_palette().background.neutral.text),
        })
}

/// Add a tooltip to an element.
pub trait WidgetExt<'a, M: 'a>:
    Sized + Into<Element<'a, M, ::iced_core::Theme, ::iced_widget::Renderer>>
{
    /// Add given tooltip to element.
    fn with_text_tooltip(self, tooltip: impl IntoFragment<'a>) -> widget::tooltip::Tooltip<'a, M> {
        self.with_tooltip(
            widget::text(tooltip).wrapping(widget::text::Wrapping::WordOrGlyph),
            300,
        )
    }

    /// Add given rich tooltip to element.
    fn with_rich_tooltip<L: Clone>(
        self,
        tooltip: Rich<'a, L, M, ::iced_core::Theme, ::iced_widget::Renderer>,
    ) -> widget::tooltip::Tooltip<'a, M> {
        self.with_tooltip(tooltip.wrapping(widget::text::Wrapping::WordOrGlyph), 300)
    }

    /// Add given alement as a tooltip.
    fn with_tooltip(
        self,
        tooltip: impl Into<Element<'a, M>>,
        max_width: u32,
    ) -> widget::tooltip::Tooltip<'a, M> {
        widget::tooltip(
            self,
            widget::container(tooltip)
                .max_width(max_width)
                .padding(4)
                .style(|theme| {
                    widget::container::rounded_box(theme).shadow(Shadow {
                        color: Color::BLACK,
                        offset: Vector::ZERO,
                        blur_radius: 3.0,
                    })
                }),
            widget::tooltip::Position::FollowCursor,
        )
    }

    /// Wrap element with given wrapper accepting a second argument.
    fn with_wrapper<W, A, E>(self, wrapper: W, arg: A) -> E
    where
        W: FnOnce(Self, A) -> E,
    {
        wrapper(self, arg)
    }
}

impl<'a, T: 'a, M: 'a> WidgetExt<'a, M> for T where
    T: Into<Element<'a, M, ::iced_core::Theme, ::iced_widget::Renderer>>
{
}
