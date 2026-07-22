//! Oneshot channel used to send process id.

use ::std::sync::Arc;

/// Create an id oneshot channel.
#[expect(clippy::missing_panics_doc, reason = "should never panic")]
pub fn id_channel() -> (IdSender, IdReceiver) {
    let mtx = Arc::new(::smol::lock::Mutex::new(None));
    let send = mtx.try_lock_arc().expect("mutex should not be locked yet");
    (IdSender { inner: send }, IdReceiver { inner: mtx })
}

/// Send an id.
#[derive(Debug)]
pub struct IdSender {
    /// Wrapped mutex guard.
    inner: ::smol::lock::MutexGuardArc<Option<u32>>,
}

impl IdSender {
    /// Send the id.
    pub fn send(mut self, id: u32) {
        *self.inner = Some(id);
    }
}

/// Receive an id.
///
/// May be cloned, however only one receiver
/// will receive the id, after which the others will
/// receive none.
#[derive(Debug, Clone)]
pub struct IdReceiver {
    /// Wrapped mutex.
    inner: Arc<::smol::lock::Mutex<Option<u32>>>,
}

impl IdReceiver {
    /// Receive the message.
    ///
    /// If none is returned the message has
    /// either already been received or the
    /// sender was dropped.
    pub async fn recv(self) -> Option<u32> {
        self.inner.lock().await.take()
    }

    /// Receive the message, blocking until it is received.
    ///
    /// If none is returned the message has
    /// either already been received or the
    /// sender was dropped.
    pub fn recv_blocking(self) -> Option<u32> {
        self.inner.lock_blocking().take()
    }

    /// Receive the message if it has been sent.
    ///
    /// If none is returned the message has
    /// either already been received or the
    /// sender was dropped.
    ///
    /// # Errors
    /// If the message has not been sent yet
    /// or a clone of the receiver is currently
    /// receiving the message `self` is returned
    /// as the `Err` variant.
    pub fn try_recv(self) -> Result<Option<u32>, Self> {
        if let Some(mut lock) = self.inner.try_lock() {
            Ok(lock.take())
        } else {
            Err(self)
        }
    }
}
