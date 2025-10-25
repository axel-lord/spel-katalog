//! Shared data formats in use buy application.

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
