//! Client component.

use ::bytes::Bytes;
use ::http_body_util::{BodyExt, Full};
use ::hyper::{Method, Request, Response, body::Incoming, client::conn::http1};
use ::smol::{future::FutureExt, net::unix::UnixStream};
use ::smol_hyper::rt::FuturesIo;
use ::xdg::BaseDirectories;

use crate::http::ResponseCode;

/// Error returned when failing to send a message.
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

/// An incoming http response.
#[derive(Debug)]
pub struct IncomingResponse {
    /// Wrapped incoming body.
    inner: Response<Incoming>,
}
impl IncomingResponse {
    /// Convert into body of incoming message.
    ///
    /// # Errors
    /// If the body cannot be collected.
    pub async fn body(self) -> Result<Bytes, SendError> {
        Ok(self
            .inner
            .into_body()
            .collect()
            .await
            .map_err(SendError::CollectResponse)?
            .to_bytes())
    }

    /// Get response code of response.
    pub fn code(&self) -> ResponseCode {
        self.inner.status().into()
    }
}

/// Send a message to the given writer, only returning the
/// writer and response body if the message was sent successfully.
///
/// # Errors
/// If the message cannot be sent.
pub async fn send(
    mut stream: UnixStream,
    message: Bytes,
    uri: &str,
) -> Result<IncomingResponse, SendError> {
    let io = FuturesIo::new(&mut stream);
    let (mut sender, conn) = http1::handshake(io).await.map_err(SendError::Handshake)?;
    let run = async move {
        conn.await.map_err(SendError::ConnectionFailed)?;
        Err(SendError::NoResponse)
    };
    let send = async move {
        let req = Request::builder()
            .uri(uri)
            .method(Method::POST)
            .body(Full::new(message))
            .map_err(SendError::HttpRequest)?;

        let res = sender
            .send_request(req)
            .await
            .map_err(SendError::SendHttp)?;

        Ok(IncomingResponse { inner: res })
    };

    send.or(run).await
}

/// Connect to an ipc socket.
///
/// # Errors
/// If connection cannot be established.
pub async fn connect(xdg: &BaseDirectories, name: &'static str) -> Result<UnixStream, SendError> {
    let path = xdg
        .get_runtime_file(name)
        .map_err(SendError::GetRuntimeDir)?;
    UnixStream::connect(&path).await.map_err(SendError::Connect)
}
