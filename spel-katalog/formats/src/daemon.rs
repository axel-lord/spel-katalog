//! Formats used for communication with daemon.

use ::std::path::PathBuf;

use ::serde::{Deserialize, Serialize};

use crate::{NativeGame, RunMode};

/// Response returned when running a game on a daemon.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum DaemonRunResponse {
    /// A Pipe was created.
    CreatedPipe {
        /// Name of pipe.
        name: String,
        /// Path of pipe.
        path: PathBuf,
    },
}

/// Run request sent to daemon.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DaemonRunConfigRequest<S> {
    /// Config of game to run.
    pub config: NativeGame,
    /// How to run game.
    pub run_mode: RunMode,
    /// Settings to use when running game.
    pub settings: S,
}
