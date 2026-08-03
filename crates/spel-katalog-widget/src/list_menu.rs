//! [ListMenu] impl.

use ::core::{
    fmt::{self, Debug},
    mem,
};

use ::iced_core::{
    Background, Color, Element, Length::Fill, Shadow, Theme, Vector, text::IntoFragment,
};
use ::iced_widget::{self as widget, Column, Renderer};
use ::tap::Pipe;

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

/// Create a menu button.
pub fn menu_button<'a, M>(content: impl IntoFragment<'a>) -> widget::Button<'a, M> {
    widget::button(widget::text(content))
        .width(Fill)
        .padding(2)
        .style(hover_background_text_button)
}

/// A single menu item.
enum MenuItem<'a, Message> {
    /// Item is an element.
    Element(Element<'a, Message, Theme, Renderer>),
    /// Separator between elements.
    Separator,
    /// A label.
    Label(&'a str),
}

/// List menu element.
pub struct ListMenu<'a, Message> {
    /// Wrapped column.
    inner: Vec<MenuItem<'a, Message>>,
    /// Width of list.
    width: u32,
}

impl<'a, Message> From<ListMenu<'a, Message>> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
{
    fn from(value: ListMenu<'a, Message>) -> Self {
        let width = value.width;
        let mut outer = Vec::<Element<'a, Message, Theme, Renderer>>::new();
        let mut inner = Vec::<Element<'a, Message, Theme, Renderer>>::new();

        for item in value.inner {
            match item {
                MenuItem::Separator => {
                    if !inner.is_empty() {
                        outer.push(Column::from_vec(mem::take(&mut inner)).width(Fill).into());
                    }
                    outer.push(crate::rule::horizontal().into());
                }
                MenuItem::Element(element) => inner.push(element),
                MenuItem::Label(label) => inner.push(
                    widget::text(label)
                        .style(widget::text::secondary)
                        .pipe(widget::center_x)
                        .padding(2)
                        .into(),
                ),
            }
        }
        if !inner.is_empty() {
            outer.push(Column::from_vec(inner).width(Fill).into());
        }

        widget::container(
            Column::from_vec(outer)
                .spacing(2)
                .padding(4)
                .width(width)
                .align_x(::iced_core::Alignment::Center),
        )
        .style(|theme| {
            widget::container::rounded_box(theme).shadow(Shadow {
                color: Color::BLACK,
                offset: Vector::ZERO,
                blur_radius: 5.0,
            })
        })
        .into()
    }
}

impl<'a, Message> Default for ListMenu<'a, Message> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, Message> ListMenu<'a, Message> {
    /// Construct a new list menu.
    pub const fn new() -> Self {
        Self {
            inner: Vec::new(),
            width: 130,
        }
    }

    /// Set menu width.
    pub const fn width(mut self, width: u32) -> Self {
        self.width = width;
        self
    }

    /// Update inner.
    fn push(mut self, item: MenuItem<'a, Message>) -> Self {
        self.inner.push(item);
        self
    }

    /// Insert an element.
    pub fn element(self, element: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        self.push(MenuItem::Element(element.into()))
    }

    /// Insert a separator.
    pub fn separator(self) -> Self {
        self.push(MenuItem::Separator)
    }

    /// Insert a label.
    pub fn label(self, label: &'a str) -> Self {
        self.push(MenuItem::Label(label))
    }

    /// Insert a button.
    pub fn button(self, content: impl IntoFragment<'a>, on_press: impl 'a + Fn() -> Message) -> Self
    where
        Message: 'a + Clone,
    {
        self.element(menu_button(content).on_press_with(on_press))
    }

    /// Insert a button. If the condition holds true  it is enabled.
    pub fn button_if(
        self,
        condition: bool,
        content: impl IntoFragment<'a>,
        on_press: impl 'a + Fn() -> Message,
    ) -> Self
    where
        Message: 'a + Clone,
    {
        let button = menu_button(content);
        let button = if condition {
            button.on_press_with(on_press)
        } else {
            button
        };
        self.element(button)
    }
}

impl<'a, Message> ListMenu<'a, Message> where Message: 'a + Clone {}

impl<'a, Message> Debug for MenuItem<'a, Message> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MenuItem::Element(..) => f.debug_tuple("Element").finish_non_exhaustive(),
            MenuItem::Separator => f.write_str("Separator"),
            MenuItem::Label(label) => f.debug_tuple("Label").field(label).finish(),
        }
    }
}
impl<'a, Message> Debug for ListMenu<'a, Message> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListMenu")
            .field("inner", &self.inner)
            .finish()
    }
}
