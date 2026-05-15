//! [Bind] and [Symlink] impls.
#![allow(clippy::missing_docs_in_private_items)]

use ::std::path::{Path, PathBuf};

use ::derive_more::IsVariant;
use ::serde::{Deserialize, Serialize};

/// A Single bind.
#[derive(Debug, Clone, IsVariant, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Bind {
    /// Path is mirrored in sandbox.
    MirrorNamed {
        /// Source to bind.
        src: PathBuf,
    },
    /// Path is mirrored in sandbox. As a (src) tuple.
    MirrorTuple(PathBuf),
    /// Src is bound to dest in sandbox.
    AsymNamed {
        /// Source to bind.
        src: PathBuf,
        /// Where to bind src.
        dest: PathBuf,
    },
    /// Src is bound to dest in sandbox. As a (src, dest) tuple.
    AsymTuple(PathBuf, PathBuf),
}

impl Bind {
    /// Get source and destination as `[src, dest]`,
    /// If mirrored `src` is used for both.
    pub fn normalize(&self) -> [&Path; 2] {
        match self {
            Bind::MirrorNamed { src } | Bind::MirrorTuple(src) => [src, src],
            Bind::AsymNamed { src, dest } | Bind::AsymTuple(src, dest) => [src, dest],
        }
    }
}

/// Representation of a symlink.
#[derive(Debug, Clone, IsVariant, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Symlink {
    /// Source and destination of symlink.
    Named {
        /// Source to link to.
        src: PathBuf,
        /// Where to place link.
        dest: PathBuf,
    },
    /// Source and destination of symlink. As a (src, dest) tuple.
    Tuple(PathBuf, PathBuf),
}

impl Symlink {
    /// Get source and destination as `[src, dest]`,
    pub fn normalize(&self) -> [&Path; 2] {
        let (Self::Named { src, dest } | Self::Tuple(src, dest)) = self;
        [src, dest]
    }
}
