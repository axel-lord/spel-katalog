//! Client component.

use ::bytes::Bytes;
use ::http_body_util::Full;
use ::hyper::{Request, client::conn::http1};
use ::smol::{future::FutureExt, net::unix::UnixStream};
use ::smol_hyper::rt::FuturesIo;
use ::xdg::BaseDirectories;

use crate::{
    error::{GetError, PostError, SendError},
    http::{HttpMethod, IncomingResponse},
};

/// An ipc sender, used to send a single message.
#[derive(Debug)]
pub struct IpcSender {
    /// Wrapped unix stream used to send message.
    stream: UnixStream,
}

impl IpcSender {
    /// Connect to an ipc socket.
    ///
    /// # Errors
    /// If connection cannot be established.
    pub async fn connect(xdg: &BaseDirectories, name: &str) -> Result<Self, SendError> {
        let path = xdg
            .get_runtime_file(name)
            .map_err(SendError::GetRuntimeDir)?;
        Ok(Self {
            stream: UnixStream::connect(&path)
                .await
                .map_err(SendError::Connect)?,
        })
    }

    /// Send a message consuming self and returning
    /// the response if any.
    ///
    /// # Errors
    /// If the message cannot be sent.
    pub async fn send(
        self,
        uri: &str,
        message: Bytes,
        method: HttpMethod<'_>,
    ) -> Result<IncomingResponse, SendError> {
        let Self { mut stream } = self;
        let io = FuturesIo::new(&mut stream);
        let (mut sender, conn) = http1::handshake(io).await.map_err(SendError::Handshake)?;
        let run = async move {
            conn.await.map_err(SendError::ConnectionFailed)?;
            Err(SendError::NoResponse)
        };
        let send = async move {
            let req = Request::builder()
                .uri(uri)
                .method(method)
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

    /// Send a typed message which has a body for both the request and response.
    ///
    /// # Errors
    /// If the message cannot be serialized.
    /// Or if the message cannot be sent.
    /// Or if an error response was received.
    /// Or if the response cannot be deserialized.
    pub async fn post<M>(self, message: M) -> Result<M::Response, PostError>
    where
        M: crate::Post,
    {
        let message = ::serde_json::to_vec(&message).map_err(PostError::Serialize)?;
        let response = self
            .send(&M::safe_uri(), Bytes::from_owner(message), HttpMethod::Post)
            .await?;
        let code = response.code();
        let body = response.body().await?;

        if !code.is_success() {
            return Err(if body.is_empty() {
                PostError::ResponseCode(code)
            } else {
                PostError::ResponseBody {
                    code,
                    body: String::from_utf8_lossy(&body).into_owned(),
                }
            });
        }

        ::serde_json::from_slice::<M::Response>(&body).map_err(PostError::Deserialize)
    }

    /// Send a typed get request, which has a body for the response but not the request.
    ///
    /// # Errors
    /// If the requestrequest  cannot be sent.
    /// Or if an error response was received.
    /// Or if the response cannot be deserialized.
    pub async fn get<T>(self) -> Result<T, GetError>
    where
        T: crate::Get,
    {
        let response = self
            .send(&T::safe_uri(), Bytes::new(), HttpMethod::Get)
            .await?;
        let code = response.code();
        let body = response.body().await?;

        if !code.is_success() {
            return Err(if body.is_empty() {
                GetError::ResponseCode(code)
            } else {
                GetError::ResponseBody {
                    code,
                    body: String::from_utf8_lossy(&body).into_owned(),
                }
            });
        }

        ::serde_json::from_slice::<T>(&body).map_err(GetError::Deserialize)
    }
}
