//! [NativeGame] impl.

use ::serde::{Deserialize, Serialize};
use ::uuid::Uuid;

/// Loaded short native game data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeGame {
    /// Name of the game.
    pub name: String,
    /// When was the game installed.
    pub installed_at: i64,
    /// Uuid of game.
    pub uuid: Uuid,
    /// Is the game hidden.
    pub hidden: bool,
}
