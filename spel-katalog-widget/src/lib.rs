//! Widgets with application defaults.

use ::iced_core::{Font, text::IntoFragment};

pub use self::{
    list_menu::{ListMenu, hover_background_text_button, menu_button},
    scrollable::{x_scrollable, xy_scrollable, y_scrollable},
    vertical_list_menu::VerticalListMenu,
};

pub mod rule;

mod list_menu;
mod scrollable;
mod vertical_list_menu;

pub use scrollable::y_scrollable as scrollable;

/// Display monospace text.
pub fn monospace<'a, Theme, Renderer>(
    text: impl IntoFragment<'a>,
) -> ::iced_widget::Text<'a, Theme, Renderer>
where
    Theme: 'a + ::iced_widget::text::Catalog,
    Renderer: ::iced_core::text::Renderer,
    <Renderer as ::iced_core::text::Renderer>::Font: From<::iced_core::Font>,
{
    ::iced_widget::text(text).font(Font::MONOSPACE)
}
