//! Error type wrapping strings.

use ::core::fmt::{Arguments, Display};

/// Error type converting error to a string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StrError(pub String);

impl StrError {
    /// Create formatted string errror.
    pub fn fmt(args: Arguments<'_>) -> Self {
        Self(::std::fmt::format(args))
    }
}

impl<E: ::core::error::Error> From<E> for StrError {
    fn from(value: E) -> Self {
        Self(value.to_string())
    }
}

impl Display for StrError {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        <String as Display>::fmt(&self.0, f)
    }
}
