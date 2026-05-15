//! [Timestamp] impl.

use ::core::str::FromStr;

use ::chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use ::derive_more::{AsMut, AsRef, Deref, DerefMut, Display, From, Into};
use ::serde::{Deserialize, Serialize, de};

/// Time format.
const FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// Timestamp in given locale.
#[derive(
    Debug,
    Clone,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Display,
    From,
    Into,
    Deref,
    DerefMut,
    AsRef,
    AsMut,
)]
#[repr(transparent)]
#[display("{}", _0.format(FORMAT))]
pub struct Timestamp(DateTime<Local>);

/// Error representing an error when parsing a timestamp.
#[derive(Debug, ::thiserror::Error)]
pub enum TimeStampParseError {
    /// Timestamp could not be parsed.
    #[error(transparent)]
    Parse(#[from] ::chrono::format::ParseError),
    /// Timestamp does not exist in local timezone.
    #[error("parsed timestamp does not exist")]
    Invalid,
}

impl FromStr for Timestamp {
    type Err = TimeStampParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        NaiveDateTime::parse_from_str(s, FORMAT)
            .map_err(TimeStampParseError::Parse)
            .and_then(|dt| {
                Local
                    .from_local_datetime(&dt)
                    .latest()
                    .ok_or(TimeStampParseError::Invalid)
            })
            .map(Self)
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&ToString::to_string(self))
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)
            .and_then(|s| s.parse::<Timestamp>().map_err(de::Error::custom))
    }
}
