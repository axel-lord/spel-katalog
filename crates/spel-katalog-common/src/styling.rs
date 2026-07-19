//! Styles for widgets.

use ::iced_widget as widget;

/// Bordered container with no background.
pub fn box_border(theme: &::iced_core::Theme) -> widget::container::Style {
    let mut base = widget::container::bordered_box(theme);

    base.background = None;

    base
}
