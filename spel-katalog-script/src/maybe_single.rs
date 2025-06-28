//! Serializable type that may be one or many strings.

use ::derive_more::{From, IsVariant};
use ::serde::{Deserialize, Serialize};

/// Value/s for in check.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, From, IsVariant)]
#[serde(untagged)]
pub enum MaybeSingle {
    /// Multiple values.
    Multiple(Vec<String>),
    /// A Single value.
    Single(String),
}

impl MaybeSingle {
    /// Get contents as a slice.
    pub fn as_slice(&self) -> &[String] {
        match self {
            MaybeSingle::Multiple(values) => values,
            MaybeSingle::Single(value) => ::std::slice::from_ref(value),
        }
    }

    /// Get contents as a slice.
    pub fn as_mut_slice(&mut self) -> &mut [String] {
        match self {
            MaybeSingle::Multiple(values) => values,
            MaybeSingle::Single(value) => ::std::slice::from_mut(value),
        }
    }

    /// Sort and deduplicate contents
    pub fn dedup(mut self) -> Self {
        if let Self::Multiple(values) = &mut self {
            values.sort_unstable();
            values.dedup();
        }
        self
    }
}
