//! [VerticalListMenu] impl.

use ::iced_core::text::IntoFragment;

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
