//! Channel-like types with a single sender and multiple receivers where the message may be
//! sent once, and is otherwise sent when sender goes out of scope.

use ::std::sync::Arc;

use ::smol::lock::MutexGuardArc;

/// Create a oneshot broadcast channel.
pub fn oneshot_broadcast<T>() -> (Sender<T>, Receiver<T>) {
    let mtx = Arc::new(::smol::lock::Mutex::new(None));
    let s_mtx = mtx.lock_arc_blocking();

    (Sender { mtx: s_mtx }, Receiver { mtx })
}

/// Receiver of oneshot message.
#[derive(Debug)]
pub struct Receiver<T> {
    mtx: Arc<::smol::lock::Mutex<Option<T>>>,
}

impl<T: Clone + Send> Receiver<T> {
    fn recv_(mut lock: MutexGuardArc<Option<T>>) -> Option<T> {
        if Arc::strong_count(MutexGuardArc::source(&lock)) == 1 {
            return lock.take();
        }

        lock.clone()
    }

    /// Receive value.
    pub fn recv(self) -> Option<T> {
        let Self { mtx } = self;
        let lock = mtx.lock_arc_blocking();
        Self::recv_(lock)
    }

    /// Receive a value async.
    pub async fn recv_async(self) -> Option<T> {
        let Self { mtx } = self;
        let lock = mtx.lock_arc().await;
        Self::recv_(lock)
    }
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        Self {
            mtx: self.mtx.clone(),
        }
    }
}

/// Sender of oneshot message.
#[derive(Debug)]
pub struct Sender<T> {
    mtx: ::smol::lock::MutexGuardArc<Option<T>>,
}

impl<T> Sender<T> {
    /// Consume self and send T to receivers.
    pub fn send(self, value: T) {
        let Self { mut mtx } = self;
        *mtx = Some(value)
    }
}
