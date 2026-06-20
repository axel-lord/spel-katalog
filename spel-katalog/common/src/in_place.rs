//! In place operations for types consuming self.

use ::core::ops::ControlFlow;

use ::either::Either;
use ::iced_core::{Element, Renderer};
use ::iced_widget::{Column, Row};
use ::serde::{Deserialize, Serialize};

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

/// Trait for either widgets to convene when all
/// variants are of the same type.
pub trait Convene {
    /// Type to convene to.
    type Item;

    /// Resolve to the content of whichever variant is used.
    fn convene(self) -> Self::Item;
}

impl<T> Convene for ControlFlow<T, T> {
    type Item = T;

    fn convene(self) -> Self::Item {
        match self {
            ControlFlow::Continue(a) => a,
            ControlFlow::Break(b) => b,
        }
    }
}

impl<T> Convene for Result<T, T> {
    type Item = T;

    fn convene(self) -> Self::Item {
        match self {
            Ok(a) => a,
            Err(b) => b,
        }
    }
}

impl<T> Convene for Either<T, T> {
    type Item = T;

    fn convene(self) -> Self::Item {
        match self {
            Either::Left(a) => a,
            Either::Right(b) => b,
        }
    }
}

impl<T> Convene for Intermediary<T, T> {
    type Item = T;

    fn convene(self) -> Self::Item {
        match self {
            Intermediary::Transformed(a) => a,
            Intermediary::Unaltered(b) => b,
        }
    }
}

impl Convene for Option<()> {
    type Item = ();

    fn convene(self) -> Self::Item {}
}

/// Intermediary step in conditional pipes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Intermediary<T, S> {
    /// Value has been transformed.
    Transformed(T),
    /// Value is unchanged.
    Unaltered(S),
}

impl<T, S> Intermediary<T, S> {
    /// If value is unaltered apply `f`.
    pub fn or_else<F>(self, f: F) -> T
    where
        F: FnOnce(S) -> T,
    {
        match self {
            Intermediary::Transformed(value) => value,
            Intermediary::Unaltered(value) => f(value),
        }
    }

    /// Map the transformed variant.
    pub fn map<F, V>(self, f: F) -> Intermediary<V, S>
    where
        F: FnOnce(T) -> V,
    {
        match self {
            Intermediary::Transformed(value) => Intermediary::Transformed(f(value)),
            Intermediary::Unaltered(value) => Intermediary::Unaltered(value),
        }
    }

    /// If of the `Transformed` variant call the function with a reference
    /// to the contained value.
    pub fn inspect<F>(self, f: F) -> Self
    where
        F: FnOnce(&T),
    {
        if let Self::Transformed(value) = &self {
            f(value)
        }
        self
    }

    /// If of the `Unaltered` variant call the function with a reference
    /// to the contained value.
    pub fn inspect_unaltered<F>(self, f: F) -> Self
    where
        F: FnOnce(&S),
    {
        if let Self::Unaltered(value) = &self {
            f(value)
        }
        self
    }
}

impl<T, S> From<Intermediary<T, S>> for Result<T, S> {
    fn from(value: Intermediary<T, S>) -> Self {
        value.map(Ok).or_else(Err)
    }
}

impl<T, S> From<Intermediary<T, S>> for ControlFlow<T, S> {
    fn from(value: Intermediary<T, S>) -> Self {
        value.map(ControlFlow::Break).or_else(ControlFlow::Continue)
    }
}

/// Trait used to perform various mapping on self.
pub trait MapSelf {
    /// If option is `Some` pass the contained value to `f` along with `self`.
    fn pipe_some<F, T, V>(self, option: Option<T>, f: F) -> Intermediary<V, Self>
    where
        Self: Sized,
        F: FnOnce(Self, T) -> V,
    {
        if let Some(value) = option {
            Intermediary::Transformed(f(self, value))
        } else {
            Intermediary::Unaltered(self)
        }
    }

    /// If result is `Ok` pass the contained value to `f` along with `self`.
    fn pipe_ok<F, T, V, E>(self, result: Result<T, E>, f: F) -> Intermediary<V, Self>
    where
        Self: Sized,
        F: FnOnce(Self, T) -> V,
    {
        if let Ok(value) = result {
            Intermediary::Transformed(f(self, value))
        } else {
            Intermediary::Unaltered(self)
        }
    }

    /// If condition holds transform self using `f`.
    fn pipe_if<F, T>(self, condition: bool, f: F) -> Intermediary<T, Self>
    where
        Self: Sized,
        F: FnOnce(Self) -> T,
    {
        if condition {
            Intermediary::Transformed(f(self))
        } else {
            Intermediary::Unaltered(self)
        }
    }
}

impl<T> MapSelf for T {}
