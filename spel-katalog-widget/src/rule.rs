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
