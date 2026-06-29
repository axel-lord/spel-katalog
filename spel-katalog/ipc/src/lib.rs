//! Inter process communication.

pub use crate::send::SendError;

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
    crate::listen::listen(xdg, NAME)
}

/// Attempt to send an ipc message.
///
/// # Errors
/// If no connection can be established.
/// Or if the message cannot be serialized.
pub fn send(xdg: &::xdg::BaseDirectories, message: Message) -> Result<(), SendError> {
    _ = ::smol::block_on(async move {
        let socket = crate::send::connect(xdg, NAME).await?;
        crate::send::send(socket, message).await
    })?;
    Ok(())
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
