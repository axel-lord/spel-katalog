//! Common widgets.

use ::iced_core::{Alignment::Center, Background, Color, Element};
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
            auto_scroll,
        } = scrollable::default(theme, status);

        vertical_rail.background = None;
        horizontal_rail.background = None;

        let scale_bg = |bg: Background| -> Background {
            match bg {
                Background::Color(clr) => Background::Color(clr.scale_alpha(0.5)),
                other => other,
            }
        };

        match status {
            scrollable::Status::Active { .. } => {
                vertical_rail.scroller.background = Background::Color(Color::TRANSPARENT);
                horizontal_rail.scroller.background = Background::Color(Color::TRANSPARENT);
            }
            scrollable::Status::Hovered {
                is_horizontal_scrollbar_hovered,
                is_vertical_scrollbar_hovered,
                ..
            } => {
                vertical_rail.scroller.background = scale_bg(vertical_rail.scroller.background);
                horizontal_rail.scroller.background = scale_bg(horizontal_rail.scroller.background);
                _ = (
                    is_horizontal_scrollbar_hovered,
                    is_vertical_scrollbar_hovered,
                )
            }
            scrollable::Status::Dragged {
                is_horizontal_scrollbar_dragged,
                is_vertical_scrollbar_dragged,
                ..
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
            auto_scroll,
        }
    })
}
