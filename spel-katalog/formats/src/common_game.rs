//! [CommonGame] impl.

use ::serde::{Deserialize, Serialize};

/// Game fields common to native and lutris games.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommonGame {
    /// Name of the game.
    pub name: String,
    /// When was the game installed.
    pub installed_at: i64,
    /// Is the game hidden.
    pub hidden: bool,
}
