//! [listen_typed] impl.

use ::core::convert::Infallible;

use ::bytes::Bytes;
use ::http_body_util::Full;
use ::hyper::{Response, StatusCode, server::conn::http1, service::service_fn};
use ::smol::{LocalExecutor, net::unix::UnixListener};
use ::smol_hyper::rt::FuturesIo;
use ::uuid::Uuid;

use crate::{IncomingRequest, http::HttpResponse, typed::error::ListenerError};

/// Create the given socket in runtime directory.
///
/// # Errors
/// If creation of socket fails.
/// Or on fatal unix socket listener errors.
pub async fn create_socket(
    name: &str,
    xdg: &::xdg::BaseDirectories,
) -> Result<UnixListener, ListenerError> {
    let runtime_dir = xdg
        .get_runtime_file("")
        .map_err(ListenerError::GetRuntimeDir)?;

    if let Err(err) = ::smol::fs::create_dir_all(&runtime_dir).await {
        return Err(ListenerError::CreateRuntimeDir {
            err,
            path: runtime_dir,
        });
    }

    let temp_path = runtime_dir.join(Uuid::new_v4().to_string());
    match UnixListener::bind(&temp_path) {
        Ok(socket) => {
            let socket_path = runtime_dir.join(name);
            match ::smol::fs::rename(&temp_path, &socket_path).await {
                Ok(..) => Ok(socket),
                Err(rename_err) => Err(match ::smol::fs::remove_file(&temp_path).await {
                    Ok(..) => ListenerError::RenameSocket {
                        err: rename_err,
                        from: temp_path,
                        to: socket_path,
                    },
                    Err(remove_err) => ListenerError::RemoveTempSocket {
                        rename_err,
                        remove_err,
                        from: temp_path,
                        to: socket_path,
                    },
                }),
            }
        }
        Err(err) => Err(ListenerError::CreateSocket {
            err,
            path: temp_path,
        }),
    }
}

/// Listen on the given socket using `response` to answer incoming requests.
///
/// # Errors
/// On fatal listener errors they are returned.
#[expect(clippy::future_not_send, reason = "intended to run on a single thread")]
pub async fn listen(
    socket: UnixListener,
    response: impl AsyncFn(IncomingRequest) -> Result<Bytes, HttpResponse>,
) -> Result<Infallible, ListenerError> {
    let ex = LocalExecutor::new();
    ex.run(listen_(socket, &response, &ex)).await
}

/// Listen on to given socket using executor
#[expect(clippy::future_not_send, reason = "intended to run on a single thread")]
async fn listen_<'a>(
    socket: UnixListener,
    respond: &'a impl AsyncFn(IncomingRequest) -> Result<Bytes, HttpResponse>,
    ex: &LocalExecutor<'a>,
) -> Result<Infallible, ListenerError> {
    loop {
        let (mut stream, _) = socket.accept().await.map_err(ListenerError::Accept)?;

        ex.spawn(async move {
            let io = FuturesIo::new(&mut stream);
            let serve = http1::Builder::new().serve_connection(
                io,
                service_fn(|req| async move {
                    let incoming = IncomingRequest { inner: req };

                    match respond(incoming).await {
                        Ok(data) => Response::builder()
                            .status(StatusCode::OK)
                            .body(Full::new(data)),
                        Err(err) => err.into_http(),
                    }
                }),
            );

            if let Err(err) = serve.await {
                ::log::error!("error serving http connection\n{err}");
            }
        })
        .detach();
    }
}
