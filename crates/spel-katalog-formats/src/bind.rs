//! [Bind] and [Symlink] impls.
#![allow(clippy::missing_docs_in_private_items)]

use ::core::{iter, option};
use ::std::path::{Path, PathBuf};

use ::serde::{Deserialize, Serialize};

/// A Single bind.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Bind {
    /// Source to bind.
    pub src: PathBuf,
    /// Where to bind src.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest: Option<PathBuf>,
}

impl Bind {
    /// Get source and destination as `[src, dest]`,
    /// If mirrored `src` is used for both.
    pub fn normalize(&self) -> [&Path; 2] {
        let Self { src, dest } = self;
        [src.as_path(), dest.as_ref().unwrap_or(src)]
    }

    /// Shorthand to create a mirrored bind.
    pub const fn mirrored(src: PathBuf) -> Self {
        Self { src, dest: None }
    }

    /// Shorthand to create an asymmetric bind.
    pub const fn asymmetric(src: PathBuf, dest: PathBuf) -> Self {
        Self {
            src,
            dest: Some(dest),
        }
    }

    /// Returns an iterator over `src` and optionally `dest`.
    pub fn iter(&self) -> BindIter<&'_ Path> {
        self.into_iter()
    }

    /// Returns an iterator over `src` and optionally `dest` as mutable.
    pub fn iter_mut(&mut self) -> BindIter<&'_ mut PathBuf> {
        self.into_iter()
    }
}

/// Iterator used to iterate bind.
type BindIter<T> = iter::Chain<iter::Once<T>, option::IntoIter<T>>;

impl<'a> IntoIterator for &'a mut Bind {
    type Item = &'a mut PathBuf;

    type IntoIter = BindIter<&'a mut PathBuf>;

    fn into_iter(self) -> Self::IntoIter {
        let Bind { src, dest } = self;
        iter::once(src).chain(dest.as_mut())
    }
}

impl<'a> IntoIterator for &'a Bind {
    type Item = &'a Path;

    type IntoIter = BindIter<&'a Path>;

    fn into_iter(self) -> Self::IntoIter {
        let Bind { src, dest } = self;
        iter::once(src.as_path()).chain(dest.as_deref())
    }
}

impl IntoIterator for Bind {
    type Item = PathBuf;

    type IntoIter = BindIter<PathBuf>;

    fn into_iter(self) -> Self::IntoIter {
        let Bind { src, dest } = self;
        iter::once(src).chain(dest)
    }
}

/// Representation of a symlink.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Symlink {
    /// Source to link to.
    pub src: PathBuf,
    /// Where to place link.
    pub dest: PathBuf,
}

impl Symlink {
    /// Get source and destination as `[src, dest]`,
    pub fn normalize(&self) -> [&Path; 2] {
        let Self { src, dest } = self;
        [src, dest]
    }

    /// Iterator over `src` and `dest`.
    pub fn iter(&self) -> SymlinkIter<&'_ Path> {
        self.into_iter()
    }

    /// Iterator over `src` and `dest` as mutable.
    pub fn iter_mut(&mut self) -> SymlinkIter<&'_ mut PathBuf> {
        self.into_iter()
    }
}

/// Iterator used to iterate symlink.
type SymlinkIter<T> = ::core::array::IntoIter<T, 2>;

impl IntoIterator for Symlink {
    type Item = PathBuf;

    type IntoIter = SymlinkIter<PathBuf>;

    fn into_iter(self) -> Self::IntoIter {
        let Self { src, dest } = self;
        [src, dest].into_iter()
    }
}

impl<'a> IntoIterator for &'a Symlink {
    type Item = &'a Path;

    type IntoIter = SymlinkIter<&'a Path>;

    fn into_iter(self) -> Self::IntoIter {
        let Symlink { src, dest } = self;
        [src.as_path(), dest.as_path()].into_iter()
    }
}

impl<'a> IntoIterator for &'a mut Symlink {
    type Item = &'a mut PathBuf;

    type IntoIter = SymlinkIter<&'a mut PathBuf>;

    fn into_iter(self) -> Self::IntoIter {
        let Symlink { src, dest } = self;
        [src, dest].into_iter()
    }
}
