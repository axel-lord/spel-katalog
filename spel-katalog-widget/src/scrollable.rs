//! [Scrollable] impl.

use ::iced_core::{Background, Color, Element, Theme};
use ::iced_widget::{
    Scrollable,
    scrollable::{Direction, Scrollbar},
};

/// Apply themeing.
fn apply<'a, Message, Renderer>(
    scrollable: Scrollable<'a, Message, Theme, Renderer>,
) -> Scrollable<'a, Message, Theme, Renderer>
where
    Renderer: ::iced_core::Renderer + ::iced_core::text::Renderer,
{
    scrollable.style(|theme, status| {
        let ::iced_widget::scrollable::Style {
            container,
            mut vertical_rail,
            mut horizontal_rail,
            gap,
            auto_scroll,
        } = ::iced_widget::scrollable::default(theme, status);

        vertical_rail.background = None;
        horizontal_rail.background = None;

        let scale_bg = |bg: Background| -> Background {
            match bg {
                Background::Color(clr) => Background::Color(clr.scale_alpha(0.5)),
                other => other,
            }
        };

        match status {
            ::iced_widget::scrollable::Status::Active { .. } => {
                vertical_rail.scroller.background = Background::Color(Color::TRANSPARENT);
                horizontal_rail.scroller.background = Background::Color(Color::TRANSPARENT);
            }
            ::iced_widget::scrollable::Status::Hovered {
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
            ::iced_widget::scrollable::Status::Dragged {
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

        ::iced_widget::scrollable::Style {
            container,
            vertical_rail,
            horizontal_rail,
            gap,
            auto_scroll,
        }
    })
}

/// Create a scrollable widget.
#[expect(clippy::disallowed_methods)]
pub fn y_scrollable<'a, Message, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Scrollable<'a, Message, Theme, Renderer>
where
    Renderer: ::iced_core::Renderer + ::iced_core::text::Renderer,
{
    apply(::iced_widget::scrollable(content))
}

/// Create a scrollable widget.
pub fn x_scrollable<'a, Message, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Scrollable<'a, Message, Theme, Renderer>
where
    Renderer: ::iced_core::Renderer + ::iced_core::text::Renderer,
{
    apply(Scrollable::with_direction(
        content,
        Direction::Horizontal(Scrollbar::new()),
    ))
}

/// Create a scrollable widget.
pub fn xy_scrollable<'a, Message, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Scrollable<'a, Message, Theme, Renderer>
where
    Renderer: ::iced_core::Renderer + ::iced_core::text::Renderer,
{
    apply(Scrollable::with_direction(
        content,
        Direction::Both {
            vertical: Scrollbar::new(),
            horizontal: Scrollbar::new(),
        },
    ))
}
