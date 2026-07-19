//! [NativeGame] impl.

use ::derive_more::{Deref, DerefMut};
use ::serde::{Deserialize, Serialize};
use ::uuid::Uuid;

use crate::GameCommon;

/// Loaded short native game data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, DerefMut)]
pub struct GameNative {
    /// Uuid of game.
    pub uuid: Uuid,
    /// Is the game disabled.
    pub disabled: bool,
    /// Common game fields.
    #[serde(flatten)]
    #[deref]
    #[deref_mut]
    pub common: GameCommon,
}
