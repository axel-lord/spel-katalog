//! [DbPath] impl.

use ::core::{borrow::Borrow, hash::Hash, ops::Deref};
use ::std::{
    ffi::{OsStr, OsString},
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    sync::Arc,
};

use ::derive_more::From;

/// Path-like object with static lifetime.
#[derive(Debug, From, Clone)]
pub enum DbPath {
    /// Database path is a static path.
    Path(&'static Path),
    /// Database path is a path buffer.
    PathBuf(PathBuf),
    /// Database path is an arc.
    Arc(Arc<Path>),
}

impl DbPath {
    /// Get self as filesystem path.
    pub fn as_path(&self) -> &Path {
        match self {
            DbPath::Path(path) => path,
            DbPath::PathBuf(path_buf) => path_buf,
            DbPath::Arc(path) => path,
        }
    }

    /// Return an object implementing display.
    pub fn display(&self) -> ::std::ffi::os_str::Display<'_> {
        self.as_os_str().display()
    }
}

impl Hash for DbPath {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        self.as_path().hash(state);
    }
}

impl PartialEq for DbPath {
    fn eq(&self, other: &Self) -> bool {
        self.as_path().eq(other.as_path())
    }
}

impl Eq for DbPath {}

impl Borrow<Path> for DbPath {
    fn borrow(&self) -> &Path {
        self.as_path()
    }
}

impl PartialOrd for DbPath {
    fn partial_cmp(&self, other: &Self) -> Option<::core::cmp::Ordering> {
        self.as_path().partial_cmp(other.as_path())
    }
}

impl Deref for DbPath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.as_path()
    }
}

impl AsRef<Path> for DbPath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl AsRef<OsStr> for DbPath {
    fn as_ref(&self) -> &OsStr {
        self.as_path().as_os_str()
    }
}

impl AsRef<[u8]> for DbPath {
    fn as_ref(&self) -> &[u8] {
        self.as_os_str().as_bytes()
    }
}

impl From<&'static str> for DbPath {
    fn from(value: &'static str) -> Self {
        Self::Path(Path::new(value))
    }
}

impl From<&'static OsStr> for DbPath {
    fn from(value: &'static OsStr) -> Self {
        Self::Path(Path::new(value))
    }
}

impl From<&'static [u8]> for DbPath {
    fn from(value: &'static [u8]) -> Self {
        Self::Path(Path::new(OsStr::from_bytes(value)))
    }
}

impl From<String> for DbPath {
    fn from(value: String) -> Self {
        Self::PathBuf(PathBuf::from(value))
    }
}

impl From<OsString> for DbPath {
    fn from(value: OsString) -> Self {
        Self::PathBuf(PathBuf::from(value))
    }
}

impl From<Vec<u8>> for DbPath {
    fn from(value: Vec<u8>) -> Self {
        Self::PathBuf(PathBuf::from(OsString::from_vec(value)))
    }
}
