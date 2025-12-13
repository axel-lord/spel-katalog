//! Common types for communication across crates.

mod or_request;
mod status_sender;

use ::iced_core::{Element, Renderer};
use ::iced_widget::{Column, Row};
pub use or_request::OrRequest;
pub use status_sender::StatusSender;

pub mod lazy;
pub mod styling;
pub mod w;

/// Trait for conditional row/column push.
pub trait PushMaybe<'a, M, T, R> {
    /// Push an element if some.
    fn push_maybe(self, item: Option<impl Into<Element<'a, M, T, R>>>) -> Self;
}

impl<'a, M, T, R> PushMaybe<'a, M, T, R> for Column<'a, M, T, R>
where
    R: Renderer,
{
    fn push_maybe(self, item: Option<impl Into<Element<'a, M, T, R>>>) -> Self {
        if let Some(item) = item {
            self.push(item)
        } else {
            self
        }
    }
}

impl<'a, M, T, R> PushMaybe<'a, M, T, R> for Row<'a, M, T, R>
where
    R: Renderer,
{
    fn push_maybe(self, item: Option<impl Into<Element<'a, M, T, R>>>) -> Self {
        if let Some(item) = item {
            self.push(item)
        } else {
            self
        }
    }
}

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
