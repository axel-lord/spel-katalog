//! [ListMenu] impl.

use ::core::{
    fmt::{self, Debug},
    mem,
};
use ::std::rc::Rc;

use ::iced_core::{Color, Element, Length::Fill, Shadow, Theme, Vector, text::IntoFragment};
use ::iced_widget::{self as widget, Column, Renderer};
use ::spel_katalog_widget::rule;
use ::tap::Pipe;

use crate::{hover_background_text_button, menu_item::MenuItem};

/// Create a menu button.
fn menu_button<'a, M>(content: impl IntoFragment<'a>) -> widget::Button<'a, M> {
    widget::button(widget::text(content))
        .width(Fill)
        .padding(2)
        .style(hover_background_text_button)
}

/// List menu element.
pub struct ListMenu<'a, Message> {
    /// Wrapped column.
    inner: Vec<MenuItem<'a, Message>>,
    /// Width of list.
    width: u32,
}

impl<'a, Message> FromIterator<MenuItem<'a, Message>> for ListMenu<'a, Message> {
    fn from_iter<T: IntoIterator<Item = MenuItem<'a, Message>>>(iter: T) -> Self {
        Self {
            inner: Vec::from_iter(iter),
            ..Self::new()
        }
    }
}

impl<'a, Message> Extend<MenuItem<'a, Message>> for ListMenu<'a, Message> {
    fn extend<T: IntoIterator<Item = MenuItem<'a, Message>>>(&mut self, iter: T) {
        self.inner.extend(iter);
    }
}

impl<'a, Message> IntoIterator for ListMenu<'a, Message> {
    type Item = MenuItem<'a, Message>;

    type IntoIter = ::std::vec::IntoIter<MenuItem<'a, Message>>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
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
                    outer.push(rule::horizontal().into());
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
    pub fn push(mut self, item: MenuItem<'a, Message>) -> Self {
        self.inner.push(item);
        self
    }

    /// Join elements of two list menus.
    pub fn join(self, other: Self) -> Self {
        let width = self.width.max(other.width);

        if self.inner.is_empty() {
            return Self { width, ..other };
        }

        let mut this = Self { width, ..self };

        if other.inner.is_empty() {
            return this;
        }

        this.inner.reserve(other.inner.len());
        this.inner.extend(other.inner);

        this
    }

    /// Join elements of two list menus, adding a separator if the first menu is not empty.
    pub fn join_separated(self, other: Self) -> Self {
        let width = self.width.max(other.width);

        if self.inner.is_empty() {
            return Self { width, ..other };
        }

        let mut this = Self { width, ..self };

        if other.inner.is_empty() {
            return this;
        }

        this.inner.reserve(other.inner.len() + 1);

        this = this.separator();

        this.inner.extend(other.inner);

        this
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

    /// Map message of list items.
    pub fn map<B>(self, f: impl Fn(Message) -> B + 'a) -> ListMenu<'a, B>
    where
        B: 'a,
        Message: 'a,
    {
        let Self { inner, width } = self;
        let f = Rc::new(f);

        let inner = inner
            .into_iter()
            .map(|item| {
                let f = Rc::clone(&f);
                item.map(move |m| (*f)(m))
            })
            .collect();

        ListMenu { inner, width }
    }
}

impl<'a, Message> ListMenu<'a, Message> where Message: 'a + Clone {}
impl<'a, Message> Debug for ListMenu<'a, Message> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListMenu")
            .field("inner", &self.inner)
            .finish()
    }
}
