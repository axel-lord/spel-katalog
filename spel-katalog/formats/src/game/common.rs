//! [CommonGame] impl.

use ::rustc_hash::FxHashSet;
use ::serde::{Deserialize, Serialize};

use crate::TagId;

/// Game fields common to native and lutris games.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameCommon {
    /// Name of the game.
    pub name: String,
    /// When was the game installed.
    pub installed_at: i64,
    /// Is the game hidden.
    pub hidden: bool,
    /// Tags in use by game.
    pub tags: FxHashSet<TagId>,
}
