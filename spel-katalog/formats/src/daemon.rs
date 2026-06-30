//! Formats used for communication with daemon.

use ::std::path::PathBuf;

use ::serde::{Deserialize, Serialize};

/// Response returned when running a game on a daemon.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DaemonResponse {
    /// Path to stdout pipe.
    pub stdout_pipe: PathBuf,
    /// Path to stderr pipe.
    pub stderr_pipe: PathBuf,
}
