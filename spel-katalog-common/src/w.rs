//! Common widgets.

use ::iced_core::{Alignment::Center, Color, Element};
use ::iced_widget::{Column, Row, Scrollable, scrollable};

/// Create a column.
pub fn col<'a, M, T, R>() -> Column<'a, M, T, R>
where
    R: ::iced_core::Renderer,
{
    Column::new().spacing(3)
}

/// Create a row.
pub fn row<'a, M, T, R>() -> Row<'a, M, T, R>
where
    R: ::iced_core::Renderer,
{
    Row::new().spacing(3).align_y(Center)
}

/// Create a scrollable.
pub fn scroll<'a, M, R>(
    element: impl Into<Element<'a, M, ::iced_core::Theme, R>>,
) -> Scrollable<'a, M, ::iced_core::Theme, R>
where
    R: ::iced_core::Renderer + iced_core::text::Renderer,
{
    scrollable(element).style(|theme, status| {
        let scrollable::Style {
            container,
            mut vertical_rail,
            mut horizontal_rail,
            gap,
        } = scrollable::default(theme, status);

        vertical_rail.background = None;
        horizontal_rail.background = None;

        match status {
            scrollable::Status::Active => {
                vertical_rail.scroller.color = Color::TRANSPARENT;
                horizontal_rail.scroller.color = Color::TRANSPARENT;
            }
            scrollable::Status::Hovered {
                is_horizontal_scrollbar_hovered,
                is_vertical_scrollbar_hovered,
            } => {
                vertical_rail.scroller.color = vertical_rail.scroller.color.scale_alpha(0.5);
                horizontal_rail.scroller.color = horizontal_rail.scroller.color.scale_alpha(0.5);
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
