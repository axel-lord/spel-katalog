//! Formats used for communication with daemon.

use ::std::path::PathBuf;

use ::serde::{Deserialize, Serialize};

use crate::{NativeGame, RunMode};

/// Response returned when running a game on a daemon.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DaemonResponse {
    /// Path to stdout pipe.
    pub stdout_pipe: PathBuf,
    /// Path to stderr pipe.
    pub stderr_pipe: PathBuf,
}

/// Game run config.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum DaemonRunGame<S> {
    /// Run game using provided config.
    Config {
        /// Config of game to run.
        config: NativeGame,
        /// How to run game.
        run_mode: RunMode,
        /// Settings to use when running game.
        settings: S,
    },
}
