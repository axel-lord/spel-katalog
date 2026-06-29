//! Client component.

use ::bytes::Bytes;
use ::http_body_util::{BodyExt, Full};
use ::hyper::{Method, Request, client::conn::http1};
use ::serde::Serialize;
use ::smol::{
    future::FutureExt,
    io::{AsyncRead, AsyncWrite},
    net::unix::UnixStream,
};
use ::smol_hyper::rt::FuturesIo;
use ::xdg::BaseDirectories;

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

/// Send a message to the given writer, only returning the
/// writer and response body if the message was sent successfully.
///
/// # Errors
/// If the message cannot be sent.
pub async fn send<M, W>(mut stream: W, message: M) -> Result<(W, Option<Bytes>), SendError>
where
    M: Serialize,
    W: AsyncWrite + AsyncRead + Unpin,
{
    let message = Bytes::from_owner(::serde_json::to_vec(&message).map_err(SendError::Serialize)?);
    let io = FuturesIo::new(&mut stream);
    let (mut sender, conn) = http1::handshake(io).await.map_err(SendError::Handshake)?;
    let run = async move { conn.await.map_err(SendError::ConnectionFailed) };
    let mut body = None;
    let body_ref = &mut body;
    let send = async move {
        let req = Request::builder()
            .uri("/v1")
            .method(Method::POST)
            .body(Full::new(message))
            .map_err(SendError::HttpRequest)?;

        let res = sender
            .send_request(req)
            .await
            .map_err(SendError::SendHttp)?;

        let status = res.status();
        let body = res
            .into_body()
            .collect()
            .await
            .map_err(|err| ::log::warn!("could not collect body of response\n{err}"))
            .map(|body| body.to_bytes())
            .unwrap_or_default();

        if status.is_success() {
            ::log::info!("installer opened");
            *body_ref = Some(body);
        } else if body.is_empty() {
            ::log::error!("status: {status}")
        } else {
            if let Ok(body) = str::from_utf8(&body) {
                ::log::error!("status: {status}, body:\n{body}")
            } else {
                ::log::error!("status: {status}, body:\n{body:#?}")
            }
        }

        Ok(())
    };

    send.or(run).await?;

    Ok((stream, body))
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
