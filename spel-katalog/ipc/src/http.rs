//! Http utilities for responses and requests.

use ::core::fmt::Display;

use ::bytes::Bytes;
use ::http_body_util::Full;
use ::hyper::{Method, Response, StatusCode};
use ::tap::{Conv, Pipe};

use crate::http::private::Private;

mod private {
    //! Private module for sealed-like behaviour.

    /// Hide content of enum variants.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Private<T> {
        /// Hidden value.
        pub(crate) value: T,
    }
}

/// Error which may be converted to an http response.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// Kind of error.
    kind: ResponseCode,
    /// Error body.
    body: Bytes,
}

impl HttpResponse {
    /// Convert into an http response.
    ///
    /// # Errors
    /// If http response cannot be built.
    pub fn into_http(self) -> Result<Response<Full<Bytes>>, ::hyper::http::Error> {
        let Self { kind, body } = self;

        Response::builder()
            .status(kind.conv::<StatusCode>())
            .body(Full::new(body))
    }
}

impl From<ResponseCode> for HttpResponse {
    fn from(value: ResponseCode) -> Self {
        HttpResponse {
            kind: value,
            body: Bytes::new(),
        }
    }
}

impl<E> From<E> for HttpResponse
where
    E: Display,
{
    /// Convert any display implementor to the [ErrorResponse::Internal] variant.
    fn from(value: E) -> Self {
        let body = value.to_string().pipe(Bytes::from_owner);
        HttpResponse {
            kind: ResponseCode::Internal,
            body,
        }
    }
}

impl<T> From<HttpResponse> for Result<T, HttpResponse> {
    fn from(value: HttpResponse) -> Self {
        Err(value)
    }
}

/// Error which may be converted to an http response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseCode {
    /// Internal error.
    Internal,
    /// Not an error.
    Ok,
    /// Request was accepted.
    Accepted,
    /// Bad request from client.
    BadRequest,
    /// Method may not be used.
    MethodNotAllowed,
    /// Resource was not found.
    NotFound,
    /// Another http response.
    Other(Private<StatusCode>),
}

impl ResponseCode {
    /// Returns an object that implements [Display] for printing the [ResponseCode].
    pub fn display(self) -> impl Display {
        StatusCode::from(self)
    }

    /// Create an error response with the given kind and body.
    pub const fn with_body(self, body: Bytes) -> HttpResponse {
        HttpResponse { kind: self, body }
    }

    /// Create an error response with the given kind and body from given error.
    pub fn with_err<E: ToString>(self, err: E) -> HttpResponse {
        HttpResponse {
            kind: self,
            body: Bytes::from_owner(err.to_string()),
        }
    }

    /// Convert into a response with a body.
    ///
    /// # Errors
    /// If http cannot be built.
    pub const fn into_response<T>(self, body: Bytes) -> Result<T, HttpResponse> {
        Err(self.with_body(body))
    }

    /// Convert into an empty response.
    ///
    /// # Errors
    /// If http cannot be built.
    pub const fn into_empty_response<T>(self) -> Result<T, HttpResponse> {
        Err(self.with_body(Bytes::new()))
    }

    /// Does the code indicate success.
    pub fn is_success(self) -> bool {
        StatusCode::from(self).is_success()
    }
}

impl<T> From<ResponseCode> for Result<T, HttpResponse> {
    fn from(value: ResponseCode) -> Self {
        value.into_empty_response()
    }
}

impl From<StatusCode> for ResponseCode {
    fn from(value: StatusCode) -> Self {
        if value == StatusCode::INTERNAL_SERVER_ERROR {
            ResponseCode::Internal
        } else if value == StatusCode::OK {
            ResponseCode::Ok
        } else if value == StatusCode::ACCEPTED {
            ResponseCode::Accepted
        } else if value == StatusCode::BAD_REQUEST {
            ResponseCode::BadRequest
        } else if value == StatusCode::METHOD_NOT_ALLOWED {
            ResponseCode::MethodNotAllowed
        } else if value == StatusCode::NOT_FOUND {
            ResponseCode::NotFound
        } else {
            ResponseCode::Other(Private { value })
        }
    }
}

impl From<ResponseCode> for StatusCode {
    fn from(value: ResponseCode) -> Self {
        match value {
            ResponseCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            ResponseCode::Ok => StatusCode::OK,
            ResponseCode::Accepted => StatusCode::ACCEPTED,
            ResponseCode::BadRequest => StatusCode::BAD_REQUEST,
            ResponseCode::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            ResponseCode::NotFound => StatusCode::NOT_FOUND,
            ResponseCode::Other(code) => code.value,
        }
    }
}

/// Http method used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod<'a> {
    /// Method is POST.
    Post,
    /// Method is GET.
    Get,
    /// Method is some other method.
    Other(Private<&'a Method>),
}

impl<'a> HttpMethod<'a> {
    /// Is the method POST.
    pub fn is_post(self) -> bool {
        match self {
            HttpMethod::Post => true,
            HttpMethod::Other(method) if method.value == Method::POST => true,
            _ => false,
        }
    }

    /// Is the method GET.
    pub fn is_get(self) -> bool {
        match self {
            HttpMethod::Get => true,
            HttpMethod::Other(method) if method.value == Method::GET => true,
            _ => false,
        }
    }
}

impl<'a> From<HttpMethod<'a>> for &'a Method {
    fn from(value: HttpMethod<'a>) -> Self {
        match value {
            HttpMethod::Post => &Method::POST,
            HttpMethod::Get => &Method::GET,
            HttpMethod::Other(method) => method.value,
        }
    }
}

impl<'a> From<&'a Method> for HttpMethod<'a> {
    fn from(value: &'a Method) -> Self {
        if value == Method::POST {
            HttpMethod::Post
        } else if value == Method::GET {
            HttpMethod::Get
        } else {
            HttpMethod::Other(Private { value })
        }
    }
}

impl<'a> From<HttpMethod<'a>> for Method {
    fn from(value: HttpMethod<'a>) -> Self {
        <&'a Method>::from(value).clone()
    }
}
