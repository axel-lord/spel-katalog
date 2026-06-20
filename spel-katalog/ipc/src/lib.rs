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
    net::unix::UnixListener,
    stream::{Stream, StreamExt},
};
use ::uuid::Uuid;

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

/// Replace a unix socket listener.
async fn replace_listener(socket_path: &Path) -> Option<UnixListener> {
    let temp_path = Path::new("/tmp").join(Uuid::new_v4().to_string());
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
async fn listener(socket_path: &Path) -> Option<UnixListener> {
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

/// Internal listen function.
#[expect(clippy::future_not_send, reason = "not intended to be sent")]
async fn listen_(ex: &LocalExecutor<'_>, tx: ::flume::Sender<Message>, socket_path: &Path) {
    let Some(listener) = listener(socket_path).await else {
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

/// Get scoket name.
fn name(profile: Option<&str>) -> String {
    let profile = profile.unwrap_or("default");
    format!("spel-katalog-ipc-{profile}")
}

/// Listen for connections, using the given profile.
/// returns a stream of received messages.
pub fn listen(profile: Option<&str>) -> impl 'static + Stream<Item = Message> {
    let (tx, rx) = ::flume::bounded(16);
    let name = name(profile);

    if let Err(err) = ::std::thread::Builder::new()
        .name(name.clone())
        .spawn(move || {
            ::smol::block_on(async move {
                let socket_path = Path::new("/tmp").join(name);
                let ex = LocalExecutor::new();
                ex.run(listen_(&ex, tx, &socket_path)).await
            })
        })
    {
        ::log::error!("failed to spawn ipc thread\n{err}");
    }

    rx.into_stream()
}

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

/// Attempt to send an ipc message.
///
/// # Errors
/// If no connection can be established.
/// Or if the message cannot be serialized.
pub fn send(profile: Option<&str>, message: Message) -> Result<(), SendError> {
    let name = name(profile);
    let path = Path::new("/tmp").join(&name);
    let mut conn = ::std::os::unix::net::UnixStream::connect(&path)?;
    ::serde_json::to_writer(&mut conn, &message)?;
    conn.flush()?;
    ::log::info!("message sent to {path:?}");
    Ok(())
}
