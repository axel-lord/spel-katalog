//! [Bind] and [Symlink] impls.
#![allow(clippy::missing_docs_in_private_items)]

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
}
