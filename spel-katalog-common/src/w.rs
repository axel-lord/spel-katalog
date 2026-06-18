//! Common widgets.

use ::std::sync::Arc;

use ::iced_core::{Alignment::Center, Element};
use ::iced_widget::{self as widget, Column, Row, Scrollable};

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

/// Tmp
pub fn scroll<'a, Message, Renderer>(
    _element: impl Into<Element<'a, Message, ::iced_core::Theme, Renderer>>,
) -> Scrollable<'a, Message, ::iced_core::Theme, Renderer>
where
    Renderer: ::iced_core::Renderer + ::iced_core::text::Renderer,
{
    unimplemented!()
}

/// Set config editor content.
pub fn set_text_editor_content(w: &mut widget::text_editor::Content, content: String) {
    // Probably most correct solution.
    // self.content = widget::text_editor::Content::with_text(&content);

    // Unless performed as actions, formatting is ignored for some reason
    [
        widget::text_editor::Action::SelectAll,
        widget::text_editor::Action::Edit(widget::text_editor::Edit::Delete),
        widget::text_editor::Action::Edit(widget::text_editor::Edit::Paste(Arc::new(content))),
    ]
    .into_iter()
    .for_each(|action| w.perform(action));
}
