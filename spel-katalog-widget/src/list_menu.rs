//! [ListMenu] impl.

use ::core::fmt::Debug;

use ::derive_more::{Deref, DerefMut, From, Into};
use ::iced_core::{Background, text::IntoFragment};
use ::iced_widget::Column;

/// List menu element.
#[derive(From, Into, Deref, DerefMut)]
pub struct ListMenu<'a, Message, Theme = ::iced_core::Theme, Renderer = ::iced_widget::Renderer> {
    /// Wrapped column.
    inner: Column<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Renderer> From<ListMenu<'a, Message, ::iced_core::Theme, Renderer>>
    for ::iced_core::Element<'a, Message, ::iced_core::Theme, Renderer>
where
    Renderer: 'a + ::iced_core::Renderer,
    Message: 'a,
{
    fn from(value: ListMenu<'a, Message, ::iced_core::Theme, Renderer>) -> Self {
        ::iced_widget::container(value.inner)
            .style(::iced_widget::container::bordered_box)
            .into()
    }
}
impl<'a, Message, Theme, Renderer> Default for ListMenu<'a, Message, Theme, Renderer>
where
    Renderer: 'a + ::iced_core::Renderer,
    Message: 'a,
    Theme: 'a,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, Message, Theme, Renderer> ListMenu<'a, Message, Theme, Renderer>
where
    Renderer: 'a + ::iced_core::Renderer,
    Message: 'a,
    Theme: 'a,
{
    /// Construct a new list menu.
    pub fn new() -> Self {
        Column::new()
            .spacing(0)
            .padding(3)
            .width(120)
            .align_x(::iced_core::Alignment::Center)
            .into()
    }

    /// Insert an element.
    pub fn push(
        self,
        element: impl Into<::iced_core::Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        let Self { inner } = self;
        Self {
            inner: inner.push(element),
        }
    }

    /// Insert a separator.
    pub fn separator(self) -> Self
    where
        Theme: ::iced_widget::rule::Catalog,
    {
        self.push(crate::rule::horizontal())
    }
}

impl<'a, Message> ListMenu<'a, Message, ::iced_core::Theme, ::iced_widget::Renderer>
where
    Message: 'a + Clone,
{
    /// Insert a button.
    pub fn button(
        self,
        content: impl IntoFragment<'a>,
        on_press: impl 'a + Fn() -> Message,
    ) -> Self {
        self.push(
            ::iced_widget::button(::iced_widget::text(content))
                .on_press_with(on_press)
                .width(::iced_core::Length::Fill)
                .padding(3)
                .style(|theme, status| {
                    let ::iced_widget::button::Style {
                        background,
                        text_color,
                        border,
                        shadow,
                        snap,
                    } = ::iced_widget::button::text(theme, status);

                    let background = match status {
                        ::iced_widget::button::Status::Hovered
                        | ::iced_widget::button::Status::Pressed => {
                            Some(Background::Color(theme.palette().background))
                        }
                        _ => background,
                    };

                    ::iced_widget::button::Style {
                        background,
                        text_color,
                        border,
                        shadow,
                        snap,
                    }
                }),
        )
    }
}

impl<'a, Message, Theme, Renderer> Debug for ListMenu<'a, Message, Theme, Renderer>
where
    Column<'a, Message, Theme, Renderer>: Debug,
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.debug_struct("ListMenu")
            .field("inner", &self.inner)
            .finish()
    }
}
