//! Widgets with application defaults.

use ::iced_core::{Background, Color, Element, Theme, text::IntoFragment};
use ::iced_widget::Scrollable;

pub mod rule {
    //! Standardized rules.

    /// A horizontal rule.
    #[expect(clippy::disallowed_methods)]
    pub fn horizontal<'a, Theme>() -> ::iced_widget::Rule<'a, Theme>
    where
        Theme: ::iced_widget::rule::Catalog,
    {
        ::iced_widget::rule::horizontal(2)
    }

    /// A vertical rule.
    #[expect(clippy::disallowed_methods)]
    pub fn vertical<'a, Theme>() -> ::iced_widget::Rule<'a, Theme>
    where
        Theme: ::iced_widget::rule::Catalog,
    {
        ::iced_widget::rule::vertical(2)
    }
}

/// Create a scrollable widget.
#[expect(clippy::disallowed_methods)]
pub fn scrollable<'a, Message, Renderer>(
    element: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Scrollable<'a, Message, Theme, Renderer>
where
    Renderer: ::iced_core::Renderer + ::iced_core::text::Renderer,
{
    ::iced_widget::scrollable(element).style(|theme, status| {
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

/// A Vertical list menu, has a title and dedicated functions
/// to add stylized buttons.
pub struct VerticalListMenu<'a, Message> {
    /// Backing column.
    inner: ::iced_widget::Column<'a, Message>,
}

impl<'a, Message> VerticalListMenu<'a, Message>
where
    Message: 'a,
{
    /// Construct a new list menu with the given title.
    pub fn new(title: impl IntoFragment<'a>) -> Self {
        Self {
            inner: ::iced_widget::Column::new()
                .spacing(0)
                .padding(0)
                .width(120)
                .align_x(::iced_core::Alignment::Center)
                .push(::iced_widget::text(title)),
        }
    }

    /// Convert into a container.
    pub fn into_container(self) -> ::iced_widget::Container<'a, Message> {
        let VerticalListMenu { inner } = self;
        ::iced_widget::container(inner).style(::iced_widget::container::bordered_box)
    }
}

impl<'a, Message: 'a> From<VerticalListMenu<'a, Message>>
    for ::iced_core::Element<'a, Message, ::iced_core::Theme, ::iced_widget::Renderer>
{
    fn from(value: VerticalListMenu<'a, Message>) -> Self {
        value.into_container().into()
    }
}

impl<'a, Message> ::core::fmt::Debug for VerticalListMenu<'a, Message> {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.debug_struct("VerticalListMenu").finish_non_exhaustive()
    }
}
