//! Common types for communication across crates.

mod or_request;
mod status_sender;

pub use or_request::OrRequest;
pub use status_sender::StatusSender;

pub mod lazy;
pub mod w;

/// Create a status message.
#[macro_export]
macro_rules! status {
    ($tx:expr, $($tt:tt)+) => {
        // $crate::OrStatus::Status(format!($($tt)*))
        $crate::StatusSender::blocking_send(&$tx, format!($($tt)*))
    };
}

/// Create a status message as a future.
#[macro_export]
macro_rules! async_status {
    ($tx:expr, $($tt:tt)+) => {
        // $crate::OrStatus::Status(format!($($tt)*))
        $crate::StatusSender::send(&$tx, format!($($tt)*))
    };
}
