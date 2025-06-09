//! Common widgets.

use ::iced::{
    Alignment::Center,
    Background, Color, Element,
    widget::{Column, Row, Scrollable, scrollable},
};

/// Create a column.
pub fn col<'a, M>() -> Column<'a, M> {
    Column::new().spacing(3)
}

/// Create a row.
pub fn row<'a, M>() -> Row<'a, M> {
    Row::new().spacing(3).align_y(Center)
}

/// Create a scrollable.
pub fn scroll<'a, M>(element: impl Into<Element<'a, M>>) -> Scrollable<'a, M> {
    scrollable(element).style(|theme, status| {
        let scrollable::Style {
            container,
            mut vertical_rail,
            mut horizontal_rail,
            gap,
        } = scrollable::default(theme, status);

        match status {
            scrollable::Status::Active => {
                vertical_rail.background = None;
                horizontal_rail.background = None;
                vertical_rail.scroller.color = Color::TRANSPARENT;
                horizontal_rail.scroller.color = Color::TRANSPARENT;
            }
            scrollable::Status::Hovered {
                is_horizontal_scrollbar_hovered,
                is_vertical_scrollbar_hovered,
            } => {
                let apply = |bg: &mut Background| match bg {
                    ::iced::Background::Color(color) => *color = color.scale_alpha(0.8),
                    ::iced::Background::Gradient(_gradient) => (),
                };
                vertical_rail.background.as_mut().map(apply);
                _ = (
                    is_horizontal_scrollbar_hovered,
                    is_vertical_scrollbar_hovered,
                )
            }
            scrollable::Status::Dragged {
                is_horizontal_scrollbar_dragged,
                is_vertical_scrollbar_dragged,
            } => {
                _ = (
                    is_vertical_scrollbar_dragged,
                    is_horizontal_scrollbar_dragged,
                )
            }
        }

        scrollable::Style {
            container,
            vertical_rail,
            horizontal_rail,
            gap,
        }
    })
}
