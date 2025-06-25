//! Environment modifications.

use ::std::{ffi::OsStr, str::FromStr};

use ::bon::Builder;
use ::derive_more::Into;
use ::rustc_hash::FxHashMap;
use ::serde::{Deserialize, Serialize};

use crate::builder_push::builder_push;

/// Error returned when failing to parse a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ::thiserror::Error)]
#[error("key '{0}' may not contain equals, '='")]
pub struct KeyFromStringError<S>(S);

impl<S> KeyFromStringError<S> {
    /// Convert into the wrapped invalid key value.
    pub fn into_inner(self) -> S {
        let Self(s) = self;
        s
    }
}

impl<S> KeyFromStringError<&S>
where
    S: ToOwned + ?Sized,
{
    /// Convert into `KeyFromStringError<S::Owned>`.
    pub fn cloned(self) -> KeyFromStringError<S::Owned> {
        let Self(value) = self;
        KeyFromStringError(value.to_owned())
    }
}

impl<S> KeyFromStringError<&mut S>
where
    S: ToOwned + ?Sized,
{
    /// Convert into `KeyFromStringError<S::Owned>`.
    pub fn cloned(self) -> KeyFromStringError<S::Owned> {
        let Self(value) = self;
        KeyFromStringError(value.to_owned())
    }
}

/// Env var key, may not contain '='.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Into, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(into = "String", try_from = "String")]
pub struct Key(String);

impl TryFrom<String> for Key {
    type Error = KeyFromStringError<String>;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.contains('=') {
            Err(KeyFromStringError(value))
        } else {
            Ok(Self(value))
        }
    }
}

impl<'s> TryFrom<&'s str> for Key {
    type Error = KeyFromStringError<&'s str>;

    fn try_from(value: &'s str) -> Result<Self, Self::Error> {
        if value.contains('=') {
            Err(KeyFromStringError(value))
        } else {
            Ok(Self(value.to_owned()))
        }
    }
}

impl FromStr for Key {
    type Err = KeyFromStringError<String>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.to_owned().try_into()
    }
}

impl AsRef<OsStr> for Key {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

/// How the environment should be changed.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq, Builder)]
#[serde(rename_all = "kebab-case")]
pub struct Env {
    /// Environment variables.
    #[serde(default, skip_serializing_if = "FxHashMap::is_empty")]
    #[builder(field)]
    pub vars: FxHashMap<Key, String>,

    /// Variables to unset.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(field)]
    pub unset: Vec<Key>,

    /// Unset all variables.
    #[serde(default, skip_serializing_if = "::core::ops::Not::not")]
    #[builder(default)]
    pub unset_all: bool,
}

impl Env {
    /// Visit all parsed string values.
    pub fn visit_strings<E>(
        &mut self,
        f: impl FnMut(&mut String) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self {
            vars,
            unset: _,
            unset_all: _,
        } = self;
        vars.values_mut().try_for_each(f)
    }
}

builder_push! {
    EnvBuilder
    { unset: Key }
    { vars, var: (Key, String) }
}
