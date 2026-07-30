//! Error types.

use ::std::path::PathBuf;

use crate::http::ResponseCode;

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
    /// Error returned if socket file cannot be opened for nlink monitoring.
    #[error("could not open socket file {path:?}\n{err}")]
    OpenSocketAsFile {
        /// Reson file could not be opened.
        err: ::smol::io::Error,
        /// Path of socket file.
        path: PathBuf,
    },
    /// Error returned if socket is unlinked/replaced.
    #[error("socket of ipc listener was unlinked")]
    SocketUnlinked,
}

/// Send errors.
#[derive(Debug, thiserror::Error)]
pub enum SendError {
    /// Error returned when a message cannot be serialized.
    #[error("message could not be serialized\n{0}")]
    Serialize(::serde_json::Error),
    /// Error returned when a message cannot be sent.
    #[error("message could not be sent\n{0}")]
    Send(::std::io::Error),
    /// Error returned when socket cannot be connected to.
    #[error("could not connect to socket\n{0}")]
    Connect(::smol::io::Error),
    /// Error returned on when handshake fails.
    #[error("could not perform http1 handshake\n{0}")]
    Handshake(::hyper::Error),
    /// Error when http request cannot be created.
    #[error("could not build http request\n{0}")]
    HttpRequest(::hyper::http::Error),
    /// Error when http request cannot be sent.
    #[error("could not send http request\n{0}")]
    SendHttp(::hyper::Error),
    /// Runtime directory could not be found.
    #[error("could not get runtime directory\n{0}")]
    GetRuntimeDir(::std::io::Error),
    /// Connection failed.
    #[error("connection failed\n{0}")]
    ConnectionFailed(::hyper::Error),
    /// No response was received.
    #[error("received no response")]
    NoResponse,
    /// Response body could not be collected.
    #[error("could not collect response body\n{0}")]
    CollectResponse(::hyper::Error),
}

impl SendError {
    /// Could the reason for the error be no socket existing.
    pub fn socket_missing(&self) -> bool {
        if let SendError::Send(err) = self
            && let ::std::io::ErrorKind::NotFound | ::std::io::ErrorKind::ConnectionRefused =
                err.kind()
        {
            true
        } else {
            false
        }
    }
}

/// Post request errors.
#[derive(Debug, ::thiserror::Error)]
pub enum PostError {
    /// Error occurred when sending message or receiving response.
    #[error(transparent)]
    SendError(#[from] SendError),
    /// Error occurred trying to deserialize the response.
    #[error("could not deserialize response\n{0}")]
    Deserialize(::serde_json::Error),
    /// Error occurred trying to serialize the request.
    #[error("could not serialize request\n{0}")]
    Serialize(::serde_json::Error),
    /// Non [Ok][ResponseCode::Ok] response code.
    #[error("non Ok response code, {0:?}")]
    ResponseCode(ResponseCode),
    /// Non [Ok][ResponseCode::Ok] response code, with message body.
    #[error("non Ok response code, {code:?}\n{body}")]
    ResponseBody {
        /// Response code.
        code: ResponseCode,
        /// Body of response.
        body: String,
    },
}

impl PostError {
    /// Could the reason for the error be no socket existing.
    pub fn socket_missing(&self) -> bool {
        match self {
            Self::SendError(err) => err.socket_missing(),
            _ => false,
        }
    }
}

/// Get request errors.
#[derive(Debug, ::thiserror::Error)]
pub enum GetError {
    /// Error occurred when sending message or receiving response.
    #[error(transparent)]
    SendError(#[from] SendError),
    /// Error occurred trying to deserialize the response.
    #[error("could not deserialize response\n{0}")]
    Deserialize(::serde_json::Error),
    /// Non [Ok][ResponseCode::Ok] response code.
    #[error("non Ok response code, {0:?}")]
    ResponseCode(ResponseCode),
    /// Non [Ok][ResponseCode::Ok] response code, with message body.
    #[error("non Ok response code, {code:?}\n{body}")]
    ResponseBody {
        /// Response code.
        code: ResponseCode,
        /// Body of response.
        body: String,
    },
}

impl GetError {
    /// Could the reason for the error be no socket existing.
    pub fn socket_missing(&self) -> bool {
        match self {
            Self::SendError(err) => err.socket_missing(),
            _ => false,
        }
    }
}
