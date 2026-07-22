//! Widgets with application defaults.

use ::iced_core::{Element, Font, text::IntoFragment};
use ::iced_widget::{self as widget, text::Rich};

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
    elem: impl 'a + Into<Element<'a, M, ::iced_core::Theme, ::iced_widget::Renderer>>,
    text: impl IntoFragment<'a>,
) -> widget::tooltip::Tooltip<'a, M> {
    elem.with_tooltip(text)
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

    /// Add given rich tooltip to element.
    fn with_rich_tooltip<L: Clone>(
        self,
        tooltip: Rich<'a, L, M, ::iced_core::Theme, ::iced_widget::Renderer>,
    ) -> widget::tooltip::Tooltip<'a, M>;
}

impl<'a, T: 'a, M: 'a> WithTooltip<'a, M> for T
where
    T: Into<Element<'a, M, ::iced_core::Theme, ::iced_widget::Renderer>>,
{
    fn with_tooltip(self, tooltip: impl IntoFragment<'a>) -> widget::tooltip::Tooltip<'a, M> {
        widget::tooltip(
            self,
            widget::container(widget::text(tooltip).wrapping(widget::text::Wrapping::WordOrGlyph))
                .max_width(300)
                .padding(4)
                .style(widget::container::bordered_box),
            widget::tooltip::Position::FollowCursor,
        )
    }

    fn with_rich_tooltip<L: Clone>(
        self,
        tooltip: Rich<'a, L, M, ::iced_core::Theme, ::iced_widget::Renderer>,
    ) -> widget::tooltip::Tooltip<'a, M> {
        widget::tooltip(
            self,
            widget::container(tooltip.wrapping(widget::text::Wrapping::WordOrGlyph))
                .max_width(300)
                .padding(4)
                .style(widget::container::bordered_box),
            widget::tooltip::Position::FollowCursor,
        )
    }
}
