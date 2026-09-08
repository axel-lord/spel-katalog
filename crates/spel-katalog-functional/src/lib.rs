//! Functional extenstion traits.

/// Lazily pipe self to a function.
pub trait LazyPipe {
    /// Create a function which calls `f` with self
    /// as input when called.
    fn then_pipe<T>(self, f: impl Fn(Self) -> T) -> impl Fn() -> T
    where
        Self: Sized + Clone;

    /// Create a function producing a value of type `T` using self
    /// and chaining it to a value of type `V` using `f`.
    fn then_chain<T, V>(self, f: impl Fn(T) -> V) -> impl Fn() -> V
    where
        Self: Sized + Fn() -> T,
    {
        move || f(self())
    }
}

impl<V> LazyPipe for V {
    fn then_pipe<T>(self, f: impl Fn(Self) -> T) -> impl Fn() -> T
    where
        Self: Sized + Clone,
    {
        move || f(self.clone())
    }
}
