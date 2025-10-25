//! Tracker type to wait for a response when something has happened.

use ::std::{fmt::Debug, time::Duration};

use ::derive_more::IsVariant;
use ::tokio::{sync::oneshot::error::RecvError, time::timeout};

/// The response received by a monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, IsVariant)]
pub enum Response {
    /// The tracker went out odf scope.
    Lost,
    /// The tracker successfully finished.
    Finished,
    /// The tracker reached timout during wait.
    Timeout,
}

impl Response {
    fn from_result(result: Result<(), RecvError>) -> Self {
        match result {
            Ok(..) => Self::Finished,
            Err(..) => Self::Lost,
        }
    }
}

/// Create a [Tracker] - [Monitor] pair.
pub fn create_tracker_monitor() -> (Tracker, Monitor) {
    let (tracker, monitor) = ::tokio::sync::oneshot::channel();
    (Tracker(tracker), Monitor(monitor))
}

/// A tracker which may be attached to messages.
pub struct Tracker(::tokio::sync::oneshot::Sender<()>);

impl Tracker {
    /// Send a finished response to monitor and destruct. Never blocks.
    pub fn finish(self) {
        let Self(sender) = self;
        _ = sender.send(());
    }
}

impl Debug for Tracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Tracker").finish_non_exhaustive()
    }
}

/// A monitor for a tracker.
pub struct Monitor(::tokio::sync::oneshot::Receiver<()>);

impl Monitor {
    /// Wait for a response.
    pub async fn wait(self) -> Response {
        let Self(recv) = self;
        Response::from_result(recv.await)
    }

    /// Wait for a respones with a timout.
    pub async fn wait_timout(self, duration: Duration) -> Response {
        match timeout(duration, self.wait()).await {
            Ok(response) => response,
            Err(..) => Response::Timeout,
        }
    }

    /// Wait for a response, blocking until received.
    pub fn wait_blocking(self) -> Response {
        let Self(recv) = self;
        Response::from_result(recv.blocking_recv())
    }
}

impl Debug for Monitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Monitor").finish_non_exhaustive()
    }
}
