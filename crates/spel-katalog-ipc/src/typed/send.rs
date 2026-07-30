//! Typed senders.

use ::bytes::Bytes;
use ::smol::net::unix::UnixStream;

use crate::{
    SendError,
    http::{HttpMethod, ResponseCode},
    send::send,
};

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

/// Send a typed message which has a body for both the request and response.
///
/// # Errors
/// If the message cannot be serialized.
/// Or if the message cannot be sent.
/// Or if an error response was received.
/// Or if the response cannot be deserialized.
pub async fn post<M>(stream: UnixStream, message: M) -> Result<M::Response, PostError>
where
    M: crate::typed::Post,
{
    let message = ::serde_json::to_vec(&message).map_err(PostError::Serialize)?;
    let response = send(
        stream,
        &M::safe_uri(),
        Bytes::from_owner(message),
        HttpMethod::Post,
    )
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

/// Error returned when failing to send a typed message.
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

/// Send a typed get request, which has a body for the response but not the request.
///
/// # Errors
/// If the requestrequest  cannot be sent.
/// Or if an error response was received.
/// Or if the response cannot be deserialized.
pub async fn get<T>(stream: UnixStream) -> Result<T, GetError>
where
    T: crate::typed::Get,
{
    let response = send(stream, &T::safe_uri(), Bytes::new(), HttpMethod::Get).await?;
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
