//! Error type wrapping strings.

use ::core::{
    error::Error,
    fmt::{Arguments, Debug, Display},
};

/// Error type converting error to a string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StrError(pub String);

impl StrError {
    /// Convert into a type implementing [Error].
    pub fn into_error(self) -> impl Error {
        struct Wrap(StrError);
        impl Debug for Wrap {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                Debug::fmt(&self.0, f)
            }
        }
        impl Display for Wrap {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                Display::fmt(&self.0, f)
            }
        }
        impl Error for Wrap {}
        Wrap(self)
    }
}

impl StrError {
    /// Create formatted string errror.
    pub fn fmt(args: Arguments<'_>) -> Self {
        Self(::std::fmt::format(args))
    }
}

impl<E: Error> From<E> for StrError {
    fn from(value: E) -> Self {
        Self(value.to_string())
    }
}

impl Display for StrError {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        <String as Display>::fmt(&self.0, f)
    }
}
