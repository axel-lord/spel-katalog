//! [Runner] impl.

use ::std::{convert::Infallible, str::FromStr};

use ::derive_more::{Display, IsVariant};

/// Runner used by a game profile.
#[derive(Debug, Clone, IsVariant, Display)]
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
