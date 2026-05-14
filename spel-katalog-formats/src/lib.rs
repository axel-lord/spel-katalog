//! Shared data formats in use buy application.

use ::core::{convert::Infallible, num::NonZero, str::FromStr};
use ::std::path::{Path, PathBuf};

use ::bytes::Bytes;
use ::derive_more::{Display, IsVariant};
use ::rustc_hash::FxHashMap;
use ::serde::{Deserialize, Serialize};

/// Additional config values not used by lutris.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AdditionalConfig {
    /// Additional directories sandbox will be given read access to.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub sandbox_root: Vec<String>,

    /// Custom attributes for game.
    #[serde(skip_serializing_if = "FxHashMap::is_empty", default)]
    pub attrs: FxHashMap<String, String>,
}

/// Bytes and dimensions of an rgba image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    /// Width of image.
    pub width: u32,
    /// Height of image.
    pub height: u32,
    /// Content of image.
    pub bytes: Bytes,
}

/// Type used for [Deserialize] impl of [Bind].
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Bind_ {
    /// Path is mirrored in sandbox.
    MirrorA {
        /// Source to bind.
        src: PathBuf,
    },
    /// Path is mirrored in sandbox.
    MirrorB(PathBuf),
    /// Src is bound to dest in sandbox.
    AsymA {
        /// Source to bind.
        src: PathBuf,
        /// Where to bind src.
        dest: PathBuf,
    },
    /// Src is bound to dest in sandbox.
    AsymB(PathBuf, PathBuf),
}

/// A Single bind.
#[derive(Debug, Clone, IsVariant, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(untagged, from = "Bind_")]
pub enum Bind {
    /// Path is mirrored in sandbox.
    Mirror {
        /// Source to bind.
        src: PathBuf,
    },
    /// Src is bound to dest in sandbox.
    Asym {
        /// Source to bind.
        src: PathBuf,
        /// Where to bind src.
        dest: PathBuf,
    },
}

impl Bind {
    /// Get source and destination as `[src, dest]`,
    /// If mirrored `src` is used for both.
    pub fn normalize(&self) -> [&Path; 2] {
        match self {
            Bind::Mirror { src } => [src, src],
            Bind::Asym { src, dest } => [src, dest],
        }
    }
}

impl From<Bind_> for Bind {
    fn from(value: Bind_) -> Self {
        match value {
            Bind_::MirrorA { src } | Bind_::MirrorB(src) => Bind::Mirror { src },
            Bind_::AsymA { src, dest } | Bind_::AsymB(src, dest) => Bind::Asym { src, dest },
        }
    }
}

/// Representation of a symlink.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Drive {
    /// Source to point to.
    pub link: PathBuf,
    /// Where to place link.
    pub letter: char,
}

/// Loaded game data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct NativeGame {
    /// Title used for game.
    pub name: String,

    /// Executable of game.
    pub exe: PathBuf,

    /// Runner used for game.
    pub runner: NativeRunner,

    /// Prefix of game.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub prefix: Option<PathBuf>,

    /// Is the game hidden.
    #[serde(skip_serializing_if = "::core::ops::Not::not", default)]
    pub hidden: bool,

    /// Should net always be enabled/disabled.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub net: Option<bool>,

    /// Where to place the game relative to lutris games.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub anchor: Option<NonZero<i64>>,

    /// Environment variabnles of game.
    #[serde(skip_serializing_if = "FxHashMap::is_empty", default)]
    pub env: FxHashMap<String, String>,

    /// Custom attributes for game.
    #[serde(skip_serializing_if = "FxHashMap::is_empty", default)]
    pub attrs: FxHashMap<String, String>,

    /// Dll overrides of game.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub dll_override: Vec<String>,

    /// Winetricks verbs to apply to prefix.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub wt_verb: Vec<String>,

    /// Additional directories sandbox will be given read and write access to.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub bind: Vec<Bind>,

    /// Additional directories sandbox will be given read access to.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub ro_bind: Vec<Bind>,

    /// Drive letters to create in prefix.
    pub drive: Vec<Drive>,
}

/// Runner used for native games.
#[derive(
    Debug, Clone, IsVariant, PartialEq, Eq, PartialOrd, Ord, Hash, Display, Serialize, Deserialize,
)]
pub enum NativeRunner {
    /// Game is ran using wine.
    Wine,
    /// Game is ran as a native binary.
    Linux,
}

/// Loaded lutris game data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LutrisGame {
    /// Slug assinged in lutris.
    pub slug: String,
    /// Numeric id of game.
    pub id: i64,
    /// Title used for game.
    pub name: String,
    /// Runner in use.
    pub runner: LutrisRunner,
    /// Path to lutris yml for game.
    pub configpath: String,
    /// Is the game hidden.
    pub hidden: bool,
}

/// Runner used by a game profile.
#[derive(
    Debug, Clone, IsVariant, PartialEq, Eq, PartialOrd, Ord, Hash, Display, Serialize, Deserialize,
)]
pub enum LutrisRunner {
    /// Game uses wine.
    #[display("wine")]
    Wine,
    /// Game is native.
    #[display("linux")]
    Linux,
    /// Some other runner is used.
    #[display("{}", _0)]
    Other(String),
}

impl From<&str> for LutrisRunner {
    fn from(value: &str) -> Self {
        if value
            .chars()
            .flat_map(char::to_uppercase)
            .eq("WINE".chars())
        {
            Self::Wine
        } else if value
            .chars()
            .flat_map(char::to_uppercase)
            .eq("LINUX".chars())
        {
            Self::Linux
        } else {
            Self::Other(value.into())
        }
    }
}

impl FromStr for LutrisRunner {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(s))
    }
}

impl AsRef<str> for LutrisRunner {
    fn as_ref(&self) -> &str {
        match self {
            LutrisRunner::Wine => "wine",
            LutrisRunner::Linux => "linux",
            LutrisRunner::Other(other) => other,
        }
    }
}
