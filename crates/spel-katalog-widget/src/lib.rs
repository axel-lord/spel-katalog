//! Widgets with application defaults.

use ::iced_core::{Element, Font, text::IntoFragment};
use ::iced_widget as widget;

pub use self::{
    list_menu::{ListMenu, hover_background_text_button, menu_button},
    scrollable::{x_scrollable, xy_end_scrollable, xy_scrollable, y_end_scrollable, y_scrollable},
    vertical_list_menu::VerticalListMenu,
};

pub mod icon;
pub mod rule;

mod list_menu;
mod scrollable;
mod vertical_list_menu;

pub use scrollable::y_scrollable as scrollable;

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

/// Display element with a tooltip.
pub fn with_tooltip<'a, M: 'a>(
    elem: impl Into<Element<'a, M, ::iced_core::Theme, ::iced_widget::Renderer>>,
    text: impl IntoFragment<'a>,
) -> widget::tooltip::Tooltip<'a, M> {
    widget::tooltip(
        elem,
        widget::container(widget::text(text).wrapping(widget::text::Wrapping::WordOrGlyph))
            .max_width(300)
            .padding(4)
            .style(widget::container::bordered_box),
        widget::tooltip::Position::FollowCursor,
    )
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
pub trait WithTooltip<'a, M>: Sized {
    /// Add given tooltip to element.
    fn with_tooltip(self, tooltip: impl IntoFragment<'a>) -> widget::tooltip::Tooltip<'a, M>;
}

impl<'a, T: 'a, M: 'a> WithTooltip<'a, M> for T
where
    T: Into<Element<'a, M, ::iced_core::Theme, ::iced_widget::Renderer>>,
{
    fn with_tooltip(self, tooltip: impl IntoFragment<'a>) -> widget::tooltip::Tooltip<'a, M> {
        with_tooltip(self, tooltip)
    }
}
