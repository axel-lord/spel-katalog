use ::std::{path::Path, sync::LazyLock};

#[derive(Debug)]
pub struct Lazy(LazyLock<String>);

impl ::std::ops::Deref for Lazy {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Lazy {
    pub const fn new(f: fn() -> String) -> Self {
        Self(LazyLock::new(f))
    }

    pub fn as_str(&self) -> &str {
        self
    }

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

impl ::std::fmt::Display for Lazy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ::std::fmt::Display::fmt(self.0.as_str(), f)
    }
}
