//! Custom button styles.

use ::iced_core::{Color, Theme};
use ::iced_widget::button;

/// Create a button style with a border of the given color.
pub fn with_color(color: Color) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let mut base = button::background(theme, status);
        base.border.width = 1.5;
        base.border.color = color;
        base
    }
}

/// Danger button, for destructive action.
pub fn danger(theme: &Theme, status: button::Status) -> button::Style {
    with_color(theme.extended_palette().danger.base.color)(theme, status)
}

/// Primary button, for main action.
pub fn primary(theme: &Theme, status: button::Status) -> button::Style {
    with_color(theme.extended_palette().primary.base.color)(theme, status)
}

/// Secondary button, for complementary action.
pub fn secondary(theme: &Theme, status: button::Status) -> button::Style {
    with_color(theme.extended_palette().secondary.base.color)(theme, status)
}

/// Success button, for positive action.
pub fn success(theme: &Theme, status: button::Status) -> button::Style {
    with_color(theme.extended_palette().success.base.color)(theme, status)
}

/// Warning button, for risky action.
pub fn warning(theme: &Theme, status: button::Status) -> button::Style {
    with_color(theme.extended_palette().warning.base.color)(theme, status)
}
