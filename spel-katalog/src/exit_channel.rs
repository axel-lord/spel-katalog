use ::iced::futures::channel::oneshot::{Canceled, Receiver, Sender};

/// Sender for sending exit messages.
#[derive(Debug)]
pub struct ExitSender(Sender<()>);

impl ExitSender {
    pub fn send(self) {
        let Self(sender) = self;
        _ = sender.send(());
    }
}

/// Receiver for exit messages.
#[derive(Debug)]
pub struct ExitReceiver(Receiver<()>);

impl ExitReceiver {
    pub(crate) async fn recv(self) -> Result<(), Canceled> {
        let Self(recv) = self;
        recv.await
    }
}

/// Create a channel for sending exit messages.
pub fn exit_channel() -> (ExitSender, ExitReceiver) {
    let (tx, rx) = ::iced::futures::channel::oneshot::channel();
    (ExitSender(tx), ExitReceiver(rx))
}
