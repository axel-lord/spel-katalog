//! Api to interact with daemon.

use ::core::fmt::{Debug, Display};

use ::spel_katalog_formats::{InstallerConfig, daemon::response::Children};
use ::spel_katalog_ipc::{
    IpcSender,
    error::{GetError, PostError, SendError},
};
use ::xdg::BaseDirectories;

/// Connection failed.
pub struct ConnectError<T> {
    /// Connection error.
    pub err: SendError,
    /// Payload that was to be sent.
    pub payload: T,
}

impl<T> ConnectError<Box<T>> {
    /// Create a new boxed connect error.
    pub fn new_boxed(err: SendError, payload: T) -> Self {
        Self {
            err,
            payload: Box::new(payload),
        }
    }
}

impl ConnectError<()> {
    /// Create a new empty connect error
    pub const fn new_empty(err: SendError) -> Self {
        Self { err, payload: () }
    }
}

impl<T> Debug for ConnectError<T> {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.debug_struct("ConnectError")
            .field("err", &self.err)
            .finish_non_exhaustive()
    }
}

impl<T> Display for ConnectError<T> {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        write!(
            f,
            "could not connect to ipc channel\n{}\nis the application running?",
            self.err
        )
    }
}

impl<T> ::core::error::Error for ConnectError<T> {
    fn source(&self) -> Option<&(dyn ::core::error::Error + 'static)> {
        Some(&self.err)
    }
}

/// Installation request failed.
#[derive(Debug, ::thiserror::Error)]
pub enum InstallError {
    /// Could not connect to ipc channel.
    #[error(transparent)]
    Connect(ConnectError<Box<InstallerConfig>>),
    /// Could not send install request.
    #[error(transparent)]
    Post(PostError),
}

/// Install a game using provided config.
///
/// # Errors
/// If the application is not running
/// or the message cannot be successfully sent.
#[expect(clippy::disallowed_methods, reason = "is to be used instead")]
pub async fn install_game(
    config: InstallerConfig,
    xdg: &BaseDirectories,
) -> Result<(), InstallError> {
    match IpcSender::connect(xdg, "spel-katalog-ipc").await {
        Ok(conn) => conn.post(config).await.map_err(InstallError::Post),
        Err(err) => Err(InstallError::Connect(ConnectError::new_boxed(err, config))),
    }
}

/// Could not get ids of running games.
#[derive(Debug, ::thiserror::Error)]
pub enum GetRunningError {
    /// Could not connect to ipc channel.
    #[error(transparent)]
    Connect(ConnectError<()>),
    /// Could not send id request.
    #[error(transparent)]
    Get(GetError),
}

/// Get process ids of running games.
///
/// # Errors
/// If the daemon is not running
/// or the message cannot be successfully sent.
#[expect(clippy::disallowed_methods, reason = "is to be used instead")]
pub async fn get_running_games(xdg: &BaseDirectories) -> Result<Children, GetRunningError> {
    IpcSender::connect(xdg, "spel-katalog-daemon-ipc")
        .await
        .map_err(ConnectError::new_empty)
        .map_err(GetRunningError::Connect)?
        .get()
        .await
        .map_err(GetRunningError::Get)
}
