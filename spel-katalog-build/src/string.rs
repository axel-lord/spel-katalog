//! String manipulation functions.

use ::std::borrow::Cow;

/// Convert a string to title case.
pub fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    chars
        .next()
        .map(|first| first.to_uppercase())
        .into_iter()
        .flatten()
        .chain(chars)
        .collect()
}

/// Append a punctuation to the string if not already there.
pub fn doc_str(value: &str) -> Cow<'_, str> {
    if value.ends_with('.') {
        Cow::Borrowed(value)
    } else {
        Cow::Owned(format!("{value}."))
    }
}
