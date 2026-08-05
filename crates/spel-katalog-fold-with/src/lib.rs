//! Extensions to fold operations.

use ::core::hash::{BuildHasher, Hash};
use ::std::collections::{BTreeSet, HashSet};

/// Fold extension trait.
pub trait Fold: Sized {
    /// Fold given iterator with self as initial value.
    fn fold_with<I: IntoIterator, F: FnMut(Self, I::Item) -> Self>(self, iter: I, f: F) -> Self;
}

impl<T> Fold for T {
    #[inline]
    fn fold_with<I: IntoIterator, F: FnMut(Self, I::Item) -> Self>(self, iter: I, f: F) -> Self {
        iter.into_iter().fold(self, f)
    }
}

/// Trait for push operations that move self.
pub trait MovePush<T> {
    /// Push item and return self.
    fn move_push(self, value: T) -> Self;
}

impl<T> MovePush<T> for Vec<T> {
    #[inline]
    fn move_push(mut self, value: T) -> Self {
        self.push(value);
        self
    }
}

impl<T, S> MovePush<T> for HashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    #[inline]
    fn move_push(mut self, value: T) -> Self {
        self.insert(value);
        self
    }
}

impl<T> MovePush<T> for BTreeSet<T>
where
    T: Ord,
{
    #[inline]
    fn move_push(mut self, value: T) -> Self {
        self.insert(value);
        self
    }
}
