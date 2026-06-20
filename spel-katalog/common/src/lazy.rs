//! Lazy initialized strings.

use ::std::{path::Path, sync::LazyLock};

/// [LazyLock] of string implementing display.
#[derive(Debug)]
pub struct Lazy(LazyLock<String>);

impl ::core::ops::Deref for Lazy {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Lazy {
    /// Get a new instance.
    pub const fn new(f: fn() -> String) -> Self {
        Self(LazyLock::new(f))
    }

    /// Get value as a string slice.
    pub fn as_str(&self) -> &str {
        self
    }

    /// Get value as a path.
    pub fn as_path(&self) -> &Path {
        Path::new(self.as_str())
    }
}

impl<T> AsRef<T> for Lazy
where
    str: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.0.as_str().as_ref()
    }
}

impl ::core::fmt::Display for Lazy {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        ::core::fmt::Display::fmt(self.0.as_str(), f)
    }
}
