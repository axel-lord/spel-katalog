//! Common types for communication across crates.

mod or_request;
mod status_sender;

use ::core::ops::ControlFlow;

use ::iced_core::{Element, Renderer};
use ::iced_widget::{Column, Row};
pub use or_request::{IntoOrRequest, OrRequest};
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

/// Trait used to perform various mapping on self.
pub trait MapSelf {
    /// If option is `Some` provide it together with self to f and return the result,
    /// otherwise return self.
    fn with_some<F, T>(self, option: Option<T>, with: F) -> Self
    where
        Self: Sized,
        F: FnOnce(Self, T) -> Self,
    {
        if let Some(value) = option {
            with(self, value)
        } else {
            self
        }
    }

    /// If result is `Ok` provide it together with self to f and return the result,
    /// otherwise return self.
    fn with_ok<F, T, E>(self, option: Result<T, E>, with: F) -> Self
    where
        Self: Sized,
        F: FnOnce(Self, T) -> Self,
    {
        if let Ok(value) = option {
            with(self, value)
        } else {
            self
        }
    }

    /// If option is `Some` provide it together with self to f and return the result,
    /// otherwise pass self to default and return the result.
    fn with_some_else<F, D, T, V>(self, option: Option<T>, with: F, r#else: D) -> V
    where
        Self: Sized,
        D: FnOnce(Self) -> V,
        F: FnOnce(Self, T) -> V,
    {
        if let Some(value) = option {
            with(self, value)
        } else {
            r#else(self)
        }
    }

    /// If result is `Ok` provide it together with self to f and return the result,
    /// otherwise pass self to default and return the result.
    fn with_ok_else<F, D, T, V, E>(self, result: Result<T, E>, with: F, r#else: D) -> V
    where
        Self: Sized,
        D: FnOnce(Self) -> V,
        F: FnOnce(Self, T) -> V,
    {
        if let Ok(value) = result {
            with(self, value)
        } else {
            r#else(self)
        }
    }

    /// If the condition holds true apply f to self, otherwise
    /// return self as is.
    fn pipe_if<F>(self, condition: bool, f: F) -> Self
    where
        Self: Sized,
        F: FnOnce(Self) -> Self,
    {
        if condition { f(self) } else { self }
    }

    /// If the condition holds true apply f to self, otherwise
    /// apply default.
    fn pipe_if_else<F, D, T>(self, condition: bool, f: F, r#else: D) -> T
    where
        Self: Sized,
        F: FnOnce(Self) -> T,
        D: FnOnce(Self) -> T,
    {
        if condition { f(self) } else { r#else(self) }
    }

    /// If option is `Some` break with the result of f, otherwise
    /// continue with self.
    fn break_on_some<F, T, V>(self, option: Option<T>, f: F) -> ControlFlow<V, Self>
    where
        Self: Sized,
        F: FnOnce(Self, T) -> V,
    {
        if let Some(value) = option {
            ControlFlow::Break(f(self, value))
        } else {
            ControlFlow::Continue(self)
        }
    }

    /// If result is `Ok` break with the result of f, otherwise
    /// continue with self.
    fn break_on_ok<F, T, V, E>(self, result: Result<T, E>, f: F) -> ControlFlow<V, Self>
    where
        Self: Sized,
        F: FnOnce(Self, T) -> V,
    {
        if let Ok(value) = result {
            ControlFlow::Break(f(self, value))
        } else {
            ControlFlow::Continue(self)
        }
    }

    /// If condition is holds appy f and break with the result, otherwise
    /// continue with self.
    fn break_if<F, T>(self, condition: bool, f: F) -> ControlFlow<T, Self>
    where
        Self: Sized,
        F: FnOnce(Self) -> T,
    {
        if condition {
            ControlFlow::Break(f(self))
        } else {
            ControlFlow::Continue(self)
        }
    }
}

impl<T> MapSelf for T {}

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
