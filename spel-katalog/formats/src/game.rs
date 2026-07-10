//! Any game format.

use ::derive_more::{Deref, DerefMut, Display, From, IsVariant};
use ::serde::{Deserialize, Serialize};
use ::uuid::Uuid;

use crate::{LutrisGame, NativeGame};

/// Id of a game.
#[derive(
    Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(untagged)]
pub enum GameId {
    /// Lutris id.
    Lutris(i64),
    /// Native uuid.
    Native(Uuid),
}

/// Game which may be native or lutris.
#[derive(Debug, IsVariant, Clone, From, Serialize, Deserialize, PartialEq, Eq, Deref, DerefMut)]
#[deref(forward)]
#[deref_mut(forward)]
pub enum Game {
    /// Game is a lutris game.
    Lutris(LutrisGame),
    /// Game is a native game.
    Native(NativeGame),
}

impl Game {
    /// Get slug of game if available.
    pub fn slug(&self) -> Option<&str> {
        match self {
            Game::Lutris(lutris_game) => Some(&lutris_game.slug),
            Game::Native { .. } => None,
        }
    }

    /// Get id of game.
    pub const fn id(&self) -> GameId {
        match self {
            Self::Lutris(lutris_game) => GameId::Lutris(lutris_game.id),
            Self::Native(native_game) => GameId::Native(native_game.uuid),
        }
    }
}
