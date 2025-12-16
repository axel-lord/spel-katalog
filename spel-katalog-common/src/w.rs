//! Common widgets.

use ::iced_core::{Alignment::Center, Element};
use ::iced_widget::{Column, Row, Scrollable};

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
