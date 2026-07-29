//! Inter process communication.

use ::bytes::Bytes;

use crate::http::{HttpMethod, ResponseCode};
pub use crate::{
    listen::IncomingRequest,
    send::{IncomingResponse, SendError},
};

/// Socket name.
const NAME: &str = "spel-katalog-ipc.socket";

/// Ipc messages.
#[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
pub enum Message {
    /// Open installer for given game.
    InstallGame(::spel_katalog_formats::InstallerConfig),
}

/// Listen for connections using given runtime dir.
/// Returns a stream of received messages.
pub fn listen(
    xdg: &::xdg::BaseDirectories,
) -> impl 'static + ::smol::stream::Stream<Item = Message> {
    let (tx, rx) = ::flume::bounded(32);
    crate::listen::listen(xdg, NAME, move |incoming| {
        let tx = tx.clone();
        async move {
            if incoming.method().is_post() {
                match incoming.uri_path() {
                    "/v1" => {
                        let body = incoming.body().await?;

                        let message = ::serde_json::from_slice::<Message>(&body)
                            .map_err(|err| ResponseCode::BadRequest.with_err(err))?;

                        tx.send_async(message).await?;

                        ResponseCode::Accepted.into()
                    }
                    _ => ResponseCode::NotFound.into(),
                }
            } else {
                ResponseCode::MethodNotAllowed.into()
            }
        }
    });

    rx.into_stream()
}

/// Attempt to send an ipc message.
///
/// # Errors
/// If no connection can be established.
/// Or if the message cannot be serialized.
pub fn send(xdg: &::xdg::BaseDirectories, message: Message) -> Result<(), SendError> {
    ::smol::block_on(async move {
        let message =
            Bytes::from_owner(::serde_json::to_string(&message).map_err(SendError::Serialize)?);
        let socket = crate::send::connect(xdg, NAME).await?;
        let response = crate::send::send(socket, "/v1", message, HttpMethod::Post).await?;

        let status = response.code();
        let body = response.body().await?;

        if status.is_success() {
            ::log::info!("installer opened");
        } else if body.is_empty() {
            ::log::error!("status: {status}", status = status.display())
        } else {
            if let Ok(body) = str::from_utf8(&body) {
                ::log::error!("status: {status}, body:\n{body}", status = status.display())
            } else {
                ::log::error!(
                    "status: {status}, body:\n{body:#?}",
                    status = status.display()
                )
            }
        }
        Ok(())
    })
}

pub mod generic {
    //! Generic ipc listen and send.
    pub use crate::{
        listen::listen,
        send::{connect, send},
    };
}

pub mod typed {
    //! Typed ipc.

    mod error;
    mod listen;
    mod resolver;
    mod send;

    /// Trait implemented for post exchanges.
    pub trait Post: Exchange {
        /// Response to post.
        type Response: Serialize + DeserializeOwned;
    }

    impl<M, T> Post for M
    where
        M: daemon::Exchange<Method = daemon::Post<T>>,
        T: Serialize + DeserializeOwned,
    {
        type Response = T;
    }

    /// Trait implemented for get exchanges.
    pub trait Get: Exchange {}

    impl<T> Get for T where T: daemon::Exchange<Method = daemon::Get> {}

    use ::serde::{Serialize, de::DeserializeOwned};
    use ::spel_katalog_formats::daemon::{self, Exchange};

    /// Type alias to allow naming of method resolvers.
    pub type MethodResolver = Resolver<crate::typed::resolver::kind::Method>;

    pub use crate::typed::{
        error::ListenerError,
        listen::IpcListener,
        resolver::Resolver,
        send::{GetError, PostError, get, post},
    };
}

pub mod http;

mod listen;
mod send;
