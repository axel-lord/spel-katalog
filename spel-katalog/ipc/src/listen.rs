//! Server component.

use ::core::{convert::Infallible, fmt::Display};
use ::std::{path::Path, rc::Rc};

use ::bytes::Bytes;
use ::http_body_util::Full;
use ::hyper::{
    Request, Response, StatusCode, body::Incoming, server::conn::http1, service::service_fn,
};
use ::smol::{LocalExecutor, net::unix::UnixListener};
use ::smol_hyper::rt::FuturesIo;
use ::tap::{Conv, Pipe, TapOptional};
use ::uuid::Uuid;
use ::xdg::BaseDirectories;

/// Listen for connections using given runtime dir.
/// Returns a stream of received messages.
pub fn listen<
    H: 'static + Send + Fn(Request<Incoming>) -> F,
    F: Future<Output = Result<Bytes, HttpResponse>>,
>(
    xdg: &BaseDirectories,
    name: &'static str,
    handler: H,
) {
    let socket_path = match xdg.place_runtime_file(name) {
        Ok(path) => path,
        Err(err) => {
            ::log::error!("could not get runtime dir, or create parents\n{err}");
            return;
        }
    };

    let thread = ::std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            let ex = LocalExecutor::new();
            ex.run(listen_(&ex, &socket_path, handler))
                .pipe(::smol::block_on)
        });

    if let Err(err) = thread {
        ::log::error!("failed to spawn ipc thread\n{err}");
    }
}

/// Internal listen function.
#[expect(clippy::future_not_send, reason = "not intended to be sent")]
async fn listen_<
    H: 'static + Fn(Request<Incoming>) -> F,
    F: Future<Output = Result<Bytes, HttpResponse>>,
>(
    ex: &LocalExecutor<'_>,
    socket_path: &Path,
    handler: H,
) -> Option<Infallible> {
    let listener = listener(socket_path).await?;
    let handler = Rc::new(handler);

    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|err| ::log::error!("could not accept on unix socket\n{err}"))
            .ok()?;
        let handler = Rc::clone(&handler);

        ex.spawn(async move {
            let io = FuturesIo::new(&mut stream);
            let serve = http1::Builder::new()
                .serve_connection(io, service_fn(|req| request_handler(req, handler.as_ref())));
            if let Err(err) = serve.await {
                ::log::error!("error serving http connection\n{err}");
            }
        })
        .detach();
    }
}

/// Handler for http requests.
async fn request_handler<
    H: Fn(Request<Incoming>) -> F,
    F: Future<Output = Result<Bytes, HttpResponse>>,
>(
    req: Request<Incoming>,
    handler: H,
) -> Result<Response<Full<Bytes>>, ::hyper::http::Error> {
    match handler(req).await {
        Ok(body) => Response::builder()
            .status(StatusCode::OK)
            .body(Full::new(body)),
        Err(response) => response.into_http(),
    }
}

/// Error which may be converted to an http response.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// Kind of error.
    kind: ResponseKind,
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

impl From<ResponseKind> for HttpResponse {
    fn from(value: ResponseKind) -> Self {
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
            kind: ResponseKind::Internal,
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
pub enum ResponseKind {
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
}

impl ResponseKind {
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
}

impl<T> From<ResponseKind> for Result<T, HttpResponse> {
    fn from(value: ResponseKind) -> Self {
        value.into_empty_response()
    }
}

impl From<ResponseKind> for StatusCode {
    fn from(value: ResponseKind) -> Self {
        match value {
            ResponseKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            ResponseKind::Ok => StatusCode::OK,
            ResponseKind::Accepted => StatusCode::ACCEPTED,
            ResponseKind::BadRequest => StatusCode::BAD_REQUEST,
            ResponseKind::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            ResponseKind::NotFound => StatusCode::NOT_FOUND,
        }
    }
}

/// Replace a unix socket listener.
async fn replace_listener(socket_path: &Path) -> Option<UnixListener> {
    let runtime_dir = socket_path
        .parent()
        .tap_none(|| ::log::error!("could not get parent of {socket_path:?}"))?;
    let temp_path = runtime_dir.join(Uuid::new_v4().to_string());
    match UnixListener::bind(&temp_path) {
        Ok(listener) => match ::smol::fs::rename(&temp_path, socket_path).await {
            Ok(_) => Some(listener),
            Err(err) => {
                ::log::error!("could not rename {temp_path:?} -> {socket_path:?}\n{err}");
                match ::smol::fs::remove_file(&temp_path).await {
                    Ok(()) => None,
                    Err(err) => {
                        ::log::error!("could not remove {temp_path:?}\n{err}");
                        None
                    }
                }
            }
        },
        Err(err) => {
            ::log::error!("could not bind to {temp_path:?}\n{err}");
            None
        }
    }
}

/// Grab socket listener.
pub(crate) async fn listener(socket_path: &Path) -> Option<UnixListener> {
    match UnixListener::bind(socket_path) {
        Ok(listener) => Some(listener),
        Err(err) => match err.kind() {
            ::std::io::ErrorKind::AddrInUse => replace_listener(socket_path).await,
            _ => {
                ::log::error!("could not bind to {socket_path:?}\n{err}");
                None
            }
        },
    }
}
