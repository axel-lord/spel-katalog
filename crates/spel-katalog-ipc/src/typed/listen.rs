//! [listen_typed] impl.

use ::core::{convert::Infallible, time::Duration};
use ::std::os::unix::fs::MetadataExt;

use ::bytes::Bytes;
use ::http_body_util::Full;
use ::hyper::{Response, StatusCode, server::conn::http1, service::service_fn};
use ::rustix::fs::OFlags;
use ::smol::{
    LocalExecutor,
    fs::{File, unix::OpenOptionsExt},
    future::FutureExt,
    lock::Semaphore,
    net::unix::UnixListener,
};
use ::smol_hyper::rt::FuturesIo;
use ::uuid::Uuid;

use crate::{IncomingRequest, http::HttpResponse, typed::error::ListenerError};

/// A listener used to listen for and respond to
/// ipc messages.
#[derive(Debug)]
pub struct IpcListener {
    /// Unix socket backing listener.
    socket: UnixListener,
    /// Open file handle to socket file.
    file: File,
    /// Concurrency to allow.
    concurrency: usize,
}

impl IpcListener {
    /// Create the given listener.
    /// Socket will be placed in runtime directory.
    ///
    /// # Errors
    /// If creation of socket fails.
    /// Or on fatal unix socket listener errors.
    /// Or if the created socket file cannot be opened.
    pub async fn create(name: &str, xdg: &::xdg::BaseDirectories) -> Result<Self, ListenerError> {
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
        let socket = match UnixListener::bind(&temp_path) {
            Ok(socket) => socket,
            Err(err) => {
                return Err(ListenerError::CreateSocket {
                    err,
                    path: temp_path,
                });
            }
        };
        let socket_path = runtime_dir.join(name);
        let file = match ::smol::fs::OpenOptions::new()
            .custom_flags(i32::from_ne_bytes((OFlags::PATH).bits().to_ne_bytes()))
            .read(true)
            .open(&temp_path)
            .await
        {
            Ok(file) => file,
            Err(err) => {
                return Err(ListenerError::OpenSocketAsFile {
                    err,
                    path: temp_path,
                });
            }
        };

        let rename_err = match ::smol::fs::rename(&temp_path, &socket_path).await {
            Ok(..) => {
                return Ok(Self {
                    socket,
                    file,
                    concurrency: 12,
                });
            }
            Err(rename_err) => rename_err,
        };

        Err(match ::smol::fs::remove_file(&temp_path).await {
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
        })
    }

    /// Wait for socket to be unlinked.
    async fn wait_for_unlink(&self) -> Result<Infallible, ListenerError> {
        const INTERVAL: Duration = Duration::from_millis(200);
        loop {
            ::smol::Timer::after(INTERVAL).await;

            let meta = match self.file.metadata().await {
                Ok(meta) => meta,
                Err(err) => {
                    ::log::error!("could not get metadata for unix socket file\n{err}");
                    continue;
                }
            };
            let nlink = meta.nlink();

            if nlink < 1 {
                break Err(ListenerError::SocketUnlinked);
            }
        }
    }

    /// Listen on the given socket using `response` to answer incoming requests.
    ///
    /// # Errors
    /// On fatal listener errors they are returned.
    #[expect(clippy::future_not_send, reason = "intended to run on a single thread")]
    pub async fn listen(
        self,
        response: impl AsyncFn(IncomingRequest) -> Result<Bytes, HttpResponse>,
    ) -> Result<Infallible, ListenerError> {
        let semaphore = Semaphore::new(self.concurrency);
        let ex = LocalExecutor::new();
        ex.run(listen_(&self.socket, &response, &ex, &semaphore).race(self.wait_for_unlink()))
            .await
    }
}

/// Listen on to given socket using executor
#[expect(clippy::future_not_send, reason = "intended to run on a single thread")]
async fn listen_<'a>(
    socket: &'a UnixListener,
    respond: &'a impl AsyncFn(IncomingRequest) -> Result<Bytes, HttpResponse>,
    ex: &LocalExecutor<'a>,
    semaphore: &'a Semaphore,
) -> Result<Infallible, ListenerError> {
    loop {
        let guard = semaphore.acquire().await;
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

            drop(guard);
        })
        .detach();
    }
}
