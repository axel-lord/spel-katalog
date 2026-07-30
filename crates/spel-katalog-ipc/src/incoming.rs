//! [IncomingRequest] impl.

use ::bytes::Bytes;
use ::http_body_util::BodyExt as _;
use ::hyper::{Request, Response, body::Incoming};

use crate::{
    SendError,
    http::{HttpMethod, HttpResponse, ResponseCode},
};

/// An incoming http request.
#[derive(Debug)]
pub struct IncomingRequest {
    /// Wrapped incoming body.
    pub(crate) inner: Request<Incoming>,
}

impl IncomingRequest {
    /// Convert into body of incoming message.
    ///
    /// # Errors
    /// If the body cannot be collected.
    pub async fn body(self) -> Result<Bytes, HttpResponse> {
        Ok(self.inner.into_body().collect().await?.to_bytes())
    }

    /// Get uri path. Any trailing slashes
    /// are trimmed.
    pub fn uri_path(&self) -> &str {
        self.inner.uri().path().trim_end_matches('/')
    }

    /// Get method used.
    pub fn method(&self) -> HttpMethod<'_> {
        self.inner.method().into()
    }
}
/// An incoming http response.
#[derive(Debug)]
pub struct IncomingResponse {
    /// Wrapped incoming body.
    pub(crate) inner: Response<Incoming>,
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
