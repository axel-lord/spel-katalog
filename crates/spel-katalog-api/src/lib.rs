//! Api to interact with daemon.

use ::core::fmt::{Debug, Display};

use ::spel_katalog_formats::InstallerConfig;
use ::spel_katalog_ipc::error::{PostError, SendError};
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
    match ::spel_katalog_ipc::IpcSender::connect(xdg, "spel-katalog-ipc").await {
        Ok(conn) => conn.post(config).await.map_err(InstallError::Post),
        Err(err) => Err(InstallError::Connect(ConnectError::new_boxed(err, config))),
    }
}
