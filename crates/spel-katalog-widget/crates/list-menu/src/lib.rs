//! List menu to be used for context menus.

use ::iced_core::{Background, Theme};
use ::iced_widget::{self as widget};

mod list_menu;
mod menu_item;

pub use self::{list_menu::ListMenu, menu_item::MenuItem};

/// Button style with defined background while hovered.
pub fn hover_background_text_button(
    theme: &Theme,
    status: widget::button::Status,
) -> widget::button::Style {
    let widget::button::Style {
        background,
        text_color,
        border,
        shadow,
        snap,
    } = widget::button::text(theme, status);

    let (background, text_color) = match status {
        widget::button::Status::Hovered | widget::button::Status::Pressed => (
            Some(Background::Color(
                theme.extended_palette().primary.base.color,
            )),
            theme.extended_palette().primary.base.text,
        ),
        _ => (background, text_color),
    };

    let border = border.rounded(4);

    widget::button::Style {
        background,
        text_color,
        border,
        shadow,
        snap,
    }
}
