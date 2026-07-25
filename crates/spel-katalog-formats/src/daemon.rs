//! Formats used for communication with daemon.

use ::std::path::PathBuf;

use ::derive_more::{Deref, DerefMut};
use ::serde::{Deserialize, Serialize};

use crate::{NativeGameConfig, RunMode};

/// Response returned when running a game on a daemon.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum DaemonRunResponse {
    /// A Pipe was created.
    CreatedPipe {
        /// Name of game.
        name: String,
        /// Path of pipe.
        path: PathBuf,
        /// Pid of process.
        pid: i64,
    },
    /// Could not run game config.
    CouldNotRun {
        /// Name of game.
        name: String,
    },
}

/// Run request sent to daemon.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DaemonRunConfigRequest<S> {
    /// Config of game to run.
    pub config: NativeGameConfig,
    /// How to run game.
    pub run_mode: RunMode,
    /// Settings to use when running game.
    pub settings: S,
}

/// Response returned when asking for children of daemon.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Deserialize,
    Serialize,
    Deref,
    DerefMut,
)]
pub struct DaemonChildrenResponse {
    /// List of current children.
    #[deref]
    #[deref_mut]
    pub children: Vec<i64>,
}
