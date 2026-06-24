//! Inter process communication.

use ::std::{
    io::Write,
    path::{Path, PathBuf},
};

use ::flume::Sender;
use ::serde::{Deserialize, Serialize};
use ::smol::{
    LocalExecutor,
    io::AsyncReadExt,
    stream::{Stream, StreamExt},
};

pub use crate::send::SendError;

/// Socket name.
const NAME: &str = "spel-katalog-ipc.socket";

/// Ipc messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Open installer for given game.
    InstallGame {
        /// Game directory.
        source: PathBuf,
        /// Is game hidden.
        #[serde(default, skip_serializing_if = "::std::ops::Not::not")]
        hidden: bool,
        /// Should the game be moved.
        #[serde(default, skip_serializing_if = "::std::ops::Not::not")]
        move_game: bool,
        /// thumbnail of game.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thumbnail: Option<PathBuf>,
    },
}

/// Internal listen function.
#[expect(clippy::future_not_send, reason = "not intended to be sent")]
async fn listen_(
    ex: &LocalExecutor<'_>,
    tx: ::flume::Sender<Message>,
    socket_path: &Path,
    runtime_dir: &Path,
) {
    let Some(listener) = listen::listener(socket_path, runtime_dir).await else {
        return;
    };

    let mut incoming = listener.incoming();
    let mut tasks = Vec::new();

    ::log::info!("ipc initialized at {socket_path:?}");

    while let Some(conn) = incoming.next().await {
        let mut conn = match conn {
            Ok(conn) => conn,
            Err(err) => {
                ::log::error!("connection on {socket_path:?} failed\n{err}");
                continue;
            }
        };

        let tx = Sender::clone(&tx);
        tasks.push(ex.spawn(async move {
            let mut buf = Vec::new();
            if let Err(err) = conn.read_to_end(&mut buf).await {
                ::log::error!("failed to read all data for connection\n{err}");
                return;
            };

            let message = match ::serde_json::from_slice::<Message>(&buf) {
                Ok(message) => message,
                Err(err) => {
                    ::log::error!("failed to deserialize message\n{err}");
                    return;
                }
            };

            if let Err(err) = tx.send(message) {
                ::log::error!("failed to send message {err}");
            }
        }));
    }

    for task in tasks {
        task.await
    }
}

/// Listen for connections, using the given profile.
/// returns a stream of received messages.
pub fn listen(runtime_dir: PathBuf) -> impl 'static + Stream<Item = Message> {
    let (tx, rx) = ::flume::bounded(16);

    if let Err(err) = ::std::thread::Builder::new()
        .name(NAME.to_owned())
        .spawn(move || {
            ::smol::block_on(async move {
                let socket_path = runtime_dir.join(NAME);
                let ex = LocalExecutor::new();
                ex.run(listen_(&ex, tx, &socket_path, &runtime_dir)).await
            })
        })
    {
        ::log::error!("failed to spawn ipc thread\n{err}");
    }

    rx.into_stream()
}

/// Attempt to send an ipc message.
///
/// # Errors
/// If no connection can be established.
/// Or if the message cannot be serialized.
pub fn send(runtime_dir: &Path, message: Message) -> Result<(), SendError> {
    let path = runtime_dir.join(NAME);
    let mut conn = ::std::os::unix::net::UnixStream::connect(&path)?;
    ::serde_json::to_writer(&mut conn, &message)?;
    conn.flush()?;
    ::log::info!("message sent to {path:?}");
    Ok(())
}

mod send {
    //! Client component.

    use ::std::path::Path;

    use crate::Message;
    /// Error returned when failing to send a message.
    #[derive(Debug, thiserror::Error)]
    pub enum SendError {
        /// Error returned when a message cannot be serialized.
        #[error("message could not be serialized\n{0}")]
        Serialize(#[from] ::serde_json::Error),
        /// Error returned when a message cannot be sent.
        #[error("message could not be sent\n{0}")]
        Send(#[from] ::std::io::Error),
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

    /// Send an ipc message.
    pub async fn send(runtime_dir: &Path, message: Message) -> Result<(), SendError> {
        Ok(())
    }
}

mod listen {
    //! Server component.

    use ::core::convert::Infallible;
    use ::std::path::{Path, PathBuf};

    use ::bytes::Bytes;
    use ::flume::{Sender, bounded};
    use ::http_body_util::{BodyExt, Full};
    use ::hyper::{
        Method, Request, Response, StatusCode, body::Incoming, server::conn::http1,
        service::service_fn,
    };
    use ::smol::{LocalExecutor, net::unix::UnixListener, stream::Stream};
    use ::smol_hyper::rt::FuturesIo;
    use ::tap::Pipe;
    use ::uuid::Uuid;

    use crate::{Message, NAME};

    /// Listen for connections using given runtime dir.
    /// Returns a stream of received messages.
    pub fn listen(runtime_dir: PathBuf) -> impl 'static + Stream<Item = Message> {
        let (tx, rx) = bounded(64);

        let thread = ::std::thread::Builder::new()
            .name(NAME.to_owned())
            .spawn(move || {
                let socket_path = runtime_dir.join(NAME);
                let ex = LocalExecutor::new();
                ex.run(listen_(&ex, tx, &socket_path, &runtime_dir))
                    .pipe(::smol::block_on)
            });

        if let Err(err) = thread {
            ::log::error!("failed to spawn ipc thread\n{err}");
        }

        rx.into_stream()
    }

    /// Internal listen function.
    #[expect(clippy::future_not_send, reason = "not intended to be sent")]
    async fn listen_(
        ex: &LocalExecutor<'_>,
        tx: Sender<Message>,
        socket_path: &Path,
        runtime_dir: &Path,
    ) -> Option<Infallible> {
        let listener = listener(socket_path, runtime_dir).await?;

        loop {
            let (stream, _) = listener
                .accept()
                .await
                .map_err(|err| ::log::error!("could not accept on unix socket\n{err}"))
                .ok()?;
            let io = FuturesIo::new(stream);
            let tx = tx.clone();

            ex.spawn(async move {
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

    async fn request_handler(
        req: Request<Incoming>,
        tx: &Sender<Message>,
    ) -> Result<Response<Full<Bytes>>, ::hyper::http::Error> {
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

                    let message = match ::serde_json::from_slice::<Message>(&body) {
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
    async fn replace_listener(socket_path: &Path, runtime_dir: &Path) -> Option<UnixListener> {
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
    pub(crate) async fn listener(socket_path: &Path, runtime_dir: &Path) -> Option<UnixListener> {
        match UnixListener::bind(socket_path) {
            Ok(listener) => Some(listener),
            Err(err) => match err.kind() {
                ::std::io::ErrorKind::AddrInUse => replace_listener(socket_path, runtime_dir).await,
                _ => {
                    ::log::error!("could not bind to {socket_path:?}\n{err}");
                    None
                }
            },
        }
    }
}
