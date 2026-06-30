//! Inter process communication.

use ::bytes::Bytes;
use ::http_body_util::BodyExt;
use ::hyper::Method;

pub use crate::{
    listen::{HttpResponse, ResponseKind},
    send::SendError,
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
            if incoming.method() == Method::POST {
                match incoming.uri().path().trim_matches('/') {
                    "v1" => {
                        let body = incoming.into_body().collect().await?.to_bytes();

                        let message = ::serde_json::from_slice::<Message>(&body)
                            .map_err(|err| ResponseKind::BadRequest.with_err(err))?;

                        tx.send_async(message).await?;

                        ResponseKind::Accepted.into()
                    }
                    _ => ResponseKind::NotFound.into(),
                }
            } else {
                ResponseKind::MethodNotAllowed.into()
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
        let response = crate::send::send(socket, message, "/v1").await?;

        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .map_err(|err| ::log::warn!("could not collect body of response\n{err}"))
            .map(|body| body.to_bytes())
            .unwrap_or_default();

        if status.is_success() {
            ::log::info!("installer opened");
        } else if body.is_empty() {
            ::log::error!("status: {status}")
        } else {
            if let Ok(body) = str::from_utf8(&body) {
                ::log::error!("status: {status}, body:\n{body}")
            } else {
                ::log::error!("status: {status}, body:\n{body:#?}")
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

mod listen;
mod send;
