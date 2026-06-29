//! Server component.

use ::core::convert::Infallible;
use ::std::path::Path;

use ::bytes::Bytes;
use ::flume::{Sender, bounded};
use ::http_body_util::{BodyExt, Full};
use ::hyper::{
    Method, Request, Response, StatusCode, body::Incoming, server::conn::http1, service::service_fn,
};
use ::serde::de::DeserializeOwned;
use ::smol::{LocalExecutor, net::unix::UnixListener, stream::Stream};
use ::smol_hyper::rt::FuturesIo;
use ::tap::{Pipe, TapOptional};
use ::uuid::Uuid;
use ::xdg::BaseDirectories;

/// Listen for connections using given runtime dir.
/// Returns a stream of received messages.
pub fn listen<M>(xdg: &BaseDirectories, name: &'static str) -> impl 'static + Stream<Item = M>
where
    M: 'static + Send + DeserializeOwned,
{
    let (tx, rx) = bounded(64);

    let socket_path = match xdg.place_runtime_file(name) {
        Ok(path) => path,
        Err(err) => {
            ::log::error!("could not get runtime dir, or create parents\n{err}");
            return rx.into_stream();
        }
    };

    let thread = ::std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            let ex = LocalExecutor::new();
            ex.run(listen_(&ex, tx, &socket_path))
                .pipe(::smol::block_on)
        });

    if let Err(err) = thread {
        ::log::error!("failed to spawn ipc thread\n{err}");
    }

    rx.into_stream()
}

/// Internal listen function.
#[expect(clippy::future_not_send, reason = "not intended to be sent")]
async fn listen_<M>(ex: &LocalExecutor<'_>, tx: Sender<M>, socket_path: &Path) -> Option<Infallible>
where
    M: 'static + Send + DeserializeOwned,
{
    let listener = listener(socket_path).await?;

    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|err| ::log::error!("could not accept on unix socket\n{err}"))
            .ok()?;
        let tx = tx.clone();

        ex.spawn(async move {
            let io = FuturesIo::new(&mut stream);
            let serve = http1::Builder::new()
                .serve_connection(io, service_fn(|req| request_handler(req, &tx)));
            if let Err(err) = serve.await {
                ::log::error!("error serving http connection\n{err}");
            }
        })
        .detach();
    }
}

/// Create an empty response with the given status code.
fn empty_response(status: StatusCode) -> Result<Response<Full<Bytes>>, ::hyper::http::Error> {
    Response::builder()
        .status(status)
        .body(Full::new(Bytes::new()))
}

/// Handler for http requests.
async fn request_handler<M>(
    req: Request<Incoming>,
    tx: &Sender<M>,
) -> Result<Response<Full<Bytes>>, ::hyper::http::Error>
where
    M: 'static + Send + DeserializeOwned,
{
    if req.method() == Method::POST {
        match req.uri().path().trim_matches('/') {
            "v1" => {
                let body = match req.into_body().collect().await {
                    Ok(body) => body.to_bytes(),
                    Err(err) => {
                        ::log::error!("could not collect request body\n{err}");
                        return empty_response(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                };

                let message = match ::serde_json::from_slice::<M>(&body) {
                    Ok(message) => message,
                    Err(err) => {
                        return Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body(Full::new(Bytes::from_owner(err.to_string())));
                    }
                };

                match tx.send_async(message).await {
                    Ok(()) => empty_response(StatusCode::ACCEPTED),
                    Err(err) => {
                        ::log::error!("could not forward received message\n{err}");
                        empty_response(StatusCode::INTERNAL_SERVER_ERROR)
                    }
                }
            }
            _ => empty_response(StatusCode::NOT_FOUND),
        }
    } else {
        empty_response(StatusCode::METHOD_NOT_ALLOWED)
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
