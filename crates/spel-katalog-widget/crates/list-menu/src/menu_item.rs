//! [MenuItem] impl.

use ::core::fmt::{self, Debug};

use ::iced_core::{Element, Theme};
use ::iced_widget::Renderer;

/// A single menu item.
pub enum MenuItem<'a, Message> {
    /// Item is an element.
    Element(Element<'a, Message, Theme, Renderer>),
    /// Separator between elements.
    Separator,
    /// A label.
    Label(&'a str),
}

impl<'a, Message> MenuItem<'a, Message> {
    /// Map message.
    pub fn map<M>(self, f: impl Fn(Message) -> M + 'a) -> MenuItem<'a, M>
    where
        M: 'a,
        Message: 'a,
    {
        match self {
            MenuItem::Element(element) => MenuItem::Element(element.map(f)),
            MenuItem::Separator => MenuItem::Separator,

            MenuItem::Label(label) => MenuItem::Label(label),
        }
    }
}

impl<'a, Message> Debug for MenuItem<'a, Message> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MenuItem::Element(..) => f.debug_tuple("Element").finish_non_exhaustive(),
            MenuItem::Separator => f.write_str("Separator"),
            MenuItem::Label(label) => f.debug_tuple("Label").field(label).finish(),
        }
    }
}
