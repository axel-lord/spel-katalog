//! Run a future in a fresh thread.

use ::std::{
    io,
    thread::{self, JoinHandle},
};

use ::smol::block_on;

/// Use given async closure to produce a future to run in a
/// spawned thread.
///
/// # Errors
/// If thread spawning fails.
pub fn enthread<T: 'static + Send>(
    name: impl Into<String>,
    future: impl 'static + Send + AsyncFnOnce() -> T,
) -> io::Result<JoinHandle<T>> {
    thread::Builder::new()
        .name(name.into())
        .spawn(move || block_on(future()))
}
