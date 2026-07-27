//! Error types.

use ::std::path::PathBuf;

/// Fatal listener errors.
#[derive(Debug, ::thiserror::Error)]
pub enum ListenerError {
    /// Listener failed to accept a connection.
    #[error("could not accept connection\n{0}")]
    Accept(::smol::io::Error),
    /// Could not get runtime directory.
    #[error("could not get runtime directory\n{0}")]
    GetRuntimeDir(::std::io::Error),
    /// Could not create runtime directory.
    #[error("could not create runtime directory {path:?}\n{err}")]
    CreateRuntimeDir {
        /// Forwarded error.
        err: ::smol::io::Error,
        /// path of directory.
        path: PathBuf,
    },
    /// Could not create socket.
    #[error("could not create unix socket at {path:?}\n{err}")]
    CreateSocket {
        /// Forwarded error.
        err: ::smol::io::Error,
        /// path of socket.
        path: PathBuf,
    },
    /// Could not rename socket after creation.
    #[error("could not rename socket {from:?} -> {to:?}\n{err}")]
    RenameSocket {
        /// Forwarded error.
        err: ::smol::io::Error,
        /// Original name.
        from: PathBuf,
        /// New name.
        to: PathBuf,
    },
    /// Could not rename socket after creation, then could not remove temp file.
    #[error(
        "could not rename socket {from:?} -> {to:?}\n{rename_err}\ncould not remove socket {from}\n{remove_err}"
    )]
    RemoveTempSocket {
        /// Forwarded rename error.
        rename_err: ::smol::io::Error,

        /// Forwarded remove error.
        remove_err: ::smol::io::Error,

        /// Original name. And name of temp socket.
        from: PathBuf,

        /// New name.
        to: PathBuf,
    },
}
