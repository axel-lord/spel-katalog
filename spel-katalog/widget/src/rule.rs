//! Standardized rules.

use ::iced_core::Length;
use ::tap::Pipe;

/// A horizontal rule.
#[expect(clippy::disallowed_methods)]
pub fn horizontal<'a, Theme>() -> ::iced_widget::Rule<'a, Theme>
where
    Theme: ::iced_widget::rule::Catalog,
{
    ::iced_widget::rule::horizontal(2)
}

/// A horizontal rule with the given width.
pub fn sized_horizontal<'a, Message, Theme>(
    width: impl Into<Length>,
) -> ::iced_widget::Container<'a, Message, Theme>
where
    Theme: 'a + ::iced_widget::rule::Catalog + ::iced_widget::container::Catalog,
    Message: 'a,
{
    horizontal().pipe(::iced_widget::container).width(width)
}

/// A vertical rule.
#[expect(clippy::disallowed_methods)]
pub fn vertical<'a, Theme>() -> ::iced_widget::Rule<'a, Theme>
where
    Theme: ::iced_widget::rule::Catalog,
{
    ::iced_widget::rule::vertical(2)
}

/// A vertical rule with the given height.
pub fn sized_vertical<'a, Message, Theme>(
    width: impl Into<Length>,
) -> ::iced_widget::Container<'a, Message, Theme>
where
    Theme: 'a + ::iced_widget::rule::Catalog + ::iced_widget::container::Catalog,
    Message: 'a,
{
    vertical().pipe(::iced_widget::container).width(width)
}
