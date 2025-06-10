//! [StatusSender] impl
use ::derive_more::{From, Into};
use ::tokio::sync::mpsc::error::SendError;

/// Type used to send status messages.
#[derive(Debug, Clone, From, Into)]
pub struct StatusSender(::tokio::sync::mpsc::Sender<String>);

impl StatusSender {
    /// Send formatted content.
    pub fn send(&self, status: String) -> impl Send + Future<Output = ()> {
        async {
            if let Err(SendError(status)) = self.0.send(status).await {
                ::log::error!("failed to send status\n'{status}'");
            }
        }
    }

    /// Send formatted content in a blocking manner.
    pub fn blocking_send(&self, status: String) {
        if let Err(SendError(status)) = self.0.blocking_send(status) {
            ::log::error!("failed to send status\n'{status}'");
        }
    }
}
