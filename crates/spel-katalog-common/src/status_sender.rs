//! [StatusSender] impl
use ::derive_more::{From, Into};

/// Type used to send status messages.
#[derive(Debug, Clone, From, Into)]
pub struct StatusSender(::flume::Sender<String>);

impl StatusSender {
    /// Send formatted content.
    pub fn send(&self, status: String) -> impl Send + Future<Output = ()> {
        async {
            if let Err(::flume::SendError(status)) = self.0.send_async(status).await {
                ::log::error!("failed to send status\n'{status}'");
            }
        }
    }

    /// Send formatted content in a blocking manner.
    pub fn blocking_send(&self, status: String) {
        if let Err(::flume::SendError(status)) = self.0.send(status) {
            ::log::error!("failed to send status\n'{status}'");
        }
    }
}
