//! Module used for running games.

use ::core::fmt::Debug;

mod macros;
pub mod run_umu;
pub mod strerror;

/// Wrapper for functor called when and if a game is ran.
pub struct Callback {
    /// Boxed callback.
    callback: Box<dyn Send + FnOnce()>,
}

impl Callback {
    /// Construct a new instance from a callback.
    pub fn new(callback: impl 'static + Send + FnOnce()) -> Self {
        Self {
            callback: Box::new(callback),
        }
    }

    /// Call callback consuming instance.
    pub fn call(self) {
        let Self { callback } = self;
        callback()
    }
}

impl Debug for Callback {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str("OnRun")
    }
}
