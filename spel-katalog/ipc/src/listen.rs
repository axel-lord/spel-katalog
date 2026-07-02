//! Server component.

use ::core::convert::Infallible;
use ::std::{path::Path, rc::Rc, thread::JoinHandle};

use ::bytes::Bytes;
use ::http_body_util::{BodyExt, Full};
use ::hyper::{
    Request, Response, StatusCode, body::Incoming, server::conn::http1, service::service_fn,
};
use ::smol::{LocalExecutor, net::unix::UnixListener};
use ::smol_hyper::rt::FuturesIo;
use ::tap::{Pipe, TapOptional};
use ::uuid::Uuid;
use ::xdg::BaseDirectories;

use crate::http::{HttpMethod, HttpResponse};

/// Listen for connections using given runtime dir.
/// Returns a stream of received messages.
pub fn listen<
    H: 'static + Send + Fn(IncomingRequest) -> F,
    F: Future<Output = Result<Bytes, HttpResponse>>,
>(
    xdg: &BaseDirectories,
    name: &'static str,
    handler: H,
) -> Option<JoinHandle<()>> {
    let socket_path = match xdg.place_runtime_file(name) {
        Ok(path) => path,
        Err(err) => {
            ::log::error!("could not get runtime dir, or create parents\n{err}");
            return None;
        }
    };

    ::std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            let ex = LocalExecutor::new();
            ex.run(listen_(&ex, &socket_path, handler))
                .pipe(::smol::block_on);
        })
        .map_err(|err| ::log::error!("failed to spawn ipc thread\n{err}"))
        .ok()
}

/// Internal listen function.
#[expect(clippy::future_not_send, reason = "not intended to be sent")]
async fn listen_<
    H: 'static + Fn(IncomingRequest) -> F,
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
    H: Fn(IncomingRequest) -> F,
    F: Future<Output = Result<Bytes, HttpResponse>>,
>(
    req: Request<Incoming>,
    handler: H,
) -> Result<Response<Full<Bytes>>, ::hyper::http::Error> {
    match handler(IncomingRequest { inner: req }).await {
        Ok(body) => Response::builder()
            .status(StatusCode::OK)
            .body(Full::new(body)),
        Err(response) => response.into_http(),
    }
}

/// An incoming http request.
#[derive(Debug)]
pub struct IncomingRequest {
    /// Wrapped incoming body.
    inner: Request<Incoming>,
}

impl IncomingRequest {
    /// Convert into body of incoming message.
    ///
    /// # Errors
    /// If the body cannot be collected.
    pub async fn body(self) -> Result<Bytes, HttpResponse> {
        Ok(self.inner.into_body().collect().await?.to_bytes())
    }

    /// Get uri path. Any trailing or leading slashes
    /// are trimmed.
    pub fn uri_path(&self) -> &str {
        self.inner.uri().path().trim_matches('/')
    }

    /// Get method used.
    pub fn method(&self) -> HttpMethod<'_> {
        self.inner.method().into()
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
