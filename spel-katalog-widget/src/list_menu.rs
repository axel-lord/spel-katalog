//! [ListMenu] impl.

use ::core::fmt::Debug;

use ::iced_core::{Background, Element, text::IntoFragment};

/// Button style with defined background while hovered.
pub fn hover_background_text_button(
    theme: &::iced_core::Theme,
    status: ::iced_widget::button::Status,
) -> ::iced_widget::button::Style {
    let ::iced_widget::button::Style {
        background,
        text_color,
        border,
        shadow,
        snap,
    } = ::iced_widget::button::text(theme, status);

    let background = match status {
        ::iced_widget::button::Status::Hovered | ::iced_widget::button::Status::Pressed => {
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
}

/// Create a menu button.
pub fn menu_button<'a, M>(content: impl IntoFragment<'a>) -> ::iced_widget::Button<'a, M> {
    ::iced_widget::button(::iced_widget::text(content))
        .width(::iced_core::Length::Fill)
        .padding(3)
        .style(hover_background_text_button)
}

/// A single menu item.
enum MenuItem<'a, Message, Theme = ::iced_core::Theme, Renderer = ::iced_widget::Renderer> {
    /// Item is an element.
    Element(Element<'a, Message, Theme, Renderer>),
}

/// List menu element.
pub struct ListMenu<'a, Message, Theme = ::iced_core::Theme, Renderer = ::iced_widget::Renderer> {
    /// Wrapped column.
    inner: Vec<MenuItem<'a, Message, Theme, Renderer>>,
}

impl<'a, Message, Renderer> From<ListMenu<'a, Message, ::iced_core::Theme, Renderer>>
    for ::iced_core::Element<'a, Message, ::iced_core::Theme, Renderer>
where
    Renderer: 'a + ::iced_core::Renderer,
    Message: 'a,
{
    fn from(value: ListMenu<'a, Message, ::iced_core::Theme, Renderer>) -> Self {
        ::iced_widget::container(
            ::iced_widget::column(value.inner.into_iter().map(|item| match item {
                MenuItem::Element(element) => element,
            }))
            .spacing(0)
            .padding(3)
            .width(130)
            .align_x(::iced_core::Alignment::Center),
        )
        .style(::iced_widget::container::bordered_box)
        .into()
    }
}

impl<'a, Message, Theme, Renderer> Default for ListMenu<'a, Message, Theme, Renderer> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, Message, Theme, Renderer> ListMenu<'a, Message, Theme, Renderer> {
    /// Construct a new list menu.
    pub const fn new() -> Self {
        Self { inner: Vec::new() }
    }

    /// Insert an element.
    pub fn push(
        mut self,
        element: impl Into<::iced_core::Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.inner.push(MenuItem::Element(element.into()));
        self
    }

    /// Insert a separator.
    pub fn separator(self) -> Self
    where
        Theme: 'a + ::iced_widget::rule::Catalog,
        Renderer: 'a + ::iced_core::Renderer,
        Message: 'a,
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
        self.push(menu_button(content).on_press_with(on_press))
    }

    /// Insert a button. If the condition holds true  it is enabled.
    pub fn button_if(
        self,
        condition: bool,
        content: impl IntoFragment<'a>,
        on_press: impl 'a + Fn() -> Message,
    ) -> Self {
        let button = menu_button(content);
        let button = if condition {
            button.on_press_with(on_press)
        } else {
            button
        };
        self.push(button)
    }
}

impl<'a, Message, Theme, Renderer> Debug for MenuItem<'a, Message, Theme, Renderer>
where
    Element<'a, Message, Theme, Renderer>: Debug,
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        match self {
            MenuItem::Element(element) => f.debug_tuple("Element").field(element).finish(),
        }
    }
}
impl<'a, Message, Theme, Renderer> Debug for ListMenu<'a, Message, Theme, Renderer>
where
    Element<'a, Message, Theme, Renderer>: Debug,
{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.debug_struct("ListMenu")
            .field("inner", &self.inner)
            .finish()
    }
}
