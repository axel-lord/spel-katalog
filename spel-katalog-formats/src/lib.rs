//! Shared data formats in use buy application.

use ::std::{convert::Infallible, str::FromStr};

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

/// Loaded game data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    /// Slug assinged in lutris.
    pub slug: String,
    /// Numeric id of game.
    pub id: i64,
    /// Title used for game.
    pub name: String,
    /// Runner in use.
    pub runner: Runner,
    /// Path to lutris yml for game.
    pub configpath: String,
    /// Is the game hidden.
    pub hidden: bool,
    /// Is the game selected for batch commands.
    pub batch_selected: bool,
}

/// Runner used by a game profile.
#[derive(
    Debug, Clone, IsVariant, PartialEq, Eq, PartialOrd, Ord, Hash, Display, Serialize, Deserialize,
)]
pub enum Runner {
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

impl From<&str> for Runner {
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

impl FromStr for Runner {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(s))
    }
}

impl AsRef<str> for Runner {
    fn as_ref(&self) -> &str {
        match self {
            Runner::Wine => "wine",
            Runner::Linux => "linux",
            Runner::Other(other) => other,
        }
    }
}
