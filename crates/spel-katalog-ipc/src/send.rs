//! Client component.

use ::bytes::Bytes;
use ::http_body_util::{BodyExt, Full};
use ::hyper::{Method, Request, Response, body::Incoming, client::conn::http1};
use ::serde::{Serialize, de::DeserializeOwned};
use ::smol::{future::FutureExt, net::unix::UnixStream};
use ::smol_hyper::rt::FuturesIo;
use ::spel_katalog_formats::daemon::{Exchange, Get, Post};
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

/// Send a message to the given writer
/// consuming it and returning the response
/// if any.
///
/// # Errors
/// If the message cannot be sent.
pub async fn send(
    mut stream: UnixStream,
    uri: &str,
    message: Bytes,
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

/// Error returned when failing to send a typed message.
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
}

/// Send a typed message which has a body for both the request and response.
///
/// # Errors
/// If the message cannot be serialized.
/// Or if the message cannot be sent.
/// Or if an error response was received.
/// Or if the response cannot be deserialized.
pub fn post<M, T>(mut stream: UnixStream, message: M) -> Result<T, PostError>
where
    M: Exchange<Method = Post<T>>,
    T: Serialize + DeserializeOwned,
{
    todo!()
}

/// Error returned when failing to send a typed message.
#[derive(Debug, ::thiserror::Error)]
pub enum GetError {
    /// Error occurred when sending message or receiving response.
    #[error(transparent)]
    SendError(#[from] SendError),
    /// Error occurred trying to deserialize the response.
    #[error("could not deserialize response\n{0}")]
    Deserialize(::serde_json::Error),
}

/// Send a typed get request, which has a body for the response but not the request.
///
/// # Errors
/// If the requestrequest  cannot be sent.
/// Or if an error response was received.
/// Or if the response cannot be deserialized.
pub fn get<T>(mut stream: UnixStream) -> Result<T, GetError>
where
    T: Exchange<Method = Get>,
{
    todo!()
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
