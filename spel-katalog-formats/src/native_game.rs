//! [NativeGame] and [NativeRunner] impls.

use ::core::num::NonZero;
use ::std::path::PathBuf;

use ::derive_more::{Display, IsVariant};
use ::rustc_hash::FxHashMap;
use ::serde::{Deserialize, Serialize};

use crate::{Bind, Timestamp};

/// Loaded game data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct NativeGame {
    /// Title used for game.
    pub name: String,

    /// Date and time when game was added.
    pub timestamp: Timestamp,

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

    /// Drive letters to create in prefix.
    #[serde(skip_serializing_if = "FxHashMap::is_empty", default)]
    pub drives: FxHashMap<char, PathBuf>,

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
