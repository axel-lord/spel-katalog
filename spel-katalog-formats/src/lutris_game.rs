//! [LutrisGame] and [LutrisRunner] impls.

use ::core::{convert::Infallible, str::FromStr};

use ::derive_more::{Display, IsVariant};
use ::serde::{Deserialize, Serialize};

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
