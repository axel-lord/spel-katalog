//! Fixed size queue to be used for cache first-in first-discarded.

use ::core::mem::MaybeUninit;

/// A queue of size at most N, where pushes past N results
/// in the oldest item being discarded.
///
/// Push and pop behaviours are more similar to stacks, with
/// the last added element having priority, however on overfill
/// elements are discarded in a first-in first-out order.
#[derive(Debug)]
pub struct CacheQueue<const N: usize, T> {
    /// Queue contents.
    items: [MaybeUninit<T>; N],
    /// Size of queue.
    size: usize,
}

impl<const N: usize, T> Drop for CacheQueue<N, T> {
    fn drop(&mut self) {
        // SAFETY: size may not be higher than N and all elements up to size
        // must be initialized.
        for i in 0..self.size {
            unsafe {
                self.items.get_unchecked_mut(i).assume_init_drop();
            }
        }
    }
}

impl<const N: usize, T> Clone for CacheQueue<N, T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        let mut dest = CacheQueue::<N, T>::default();

        for i in 0..self.size {
            // SAFETY: size may not be higher than N and all elements up to size
            // must be initialized.
            unsafe {
                dest.items
                    .get_unchecked_mut(i)
                    .write(self.items.get_unchecked(i).assume_init_ref().clone());
            }
        }
        dest.size = self.size;

        dest
    }
}

impl<const N: usize, T> Default for CacheQueue<N, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, T> CacheQueue<N, T> {
    /// Create a new queue.
    pub const fn new() -> Self {
        Self {
            items: [const { MaybeUninit::uninit() }; N],
            size: 0,
        }
    }

    /// Get mutable slice of all initialized elements
    /// as [MaybeUninit].
    const fn as_mut_maybeuninit_slice(&mut self) -> &mut [MaybeUninit<T>] {
        // SAFETY: size may not be higher than N and all elements up to size
        // must be initialized.
        let (l, _) = unsafe { self.items.split_at_mut_unchecked(self.size) };
        l
    }

    /// Get slice of all initialized elements
    /// as [MaybeUninit].
    const fn as_maybeuninit_slice(&self) -> &[MaybeUninit<T>] {
        // SAFETY: size may not be higher than N and all elements up to size
        // must be initialized.
        let (l, _) = unsafe { self.items.split_at_unchecked(self.size) };
        l
    }

    /// Get content as a slice.
    pub const fn as_slice(&self) -> &[T] {
        let uninit_slice = self.as_maybeuninit_slice();
        // SAFETY: all elements of slice returned by `as_maybeuninit_slice` are initialized.
        unsafe { uninit_slice.assume_init_ref() }
    }

    /// Is the queue empty.
    pub const fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// How many items are in the queue.
    pub const fn len(&self) -> usize {
        self.size
    }

    /// Get index of element for which the function
    /// returns true.
    /// Search is performed in reverse order.
    fn index_of<F>(&self, mut cond: F) -> Option<usize>
    where
        F: for<'a> FnMut(&'a T) -> bool,
    {
        let mut items = self.as_slice();
        while let [prior @ .., t] = items {
            if cond(t) {
                return Some(prior.len());
            }
            items = prior;
        }
        None
    }

    /// Pop an element and shift items.
    ///
    /// # Safety
    /// Size of items must be greater than 0.
    unsafe fn pop_shift(mut items: &mut [MaybeUninit<T>]) -> T {
        // SAFETY: If size of items is greater than 0 an element exists at index 0.
        let popped = unsafe { items.get_unchecked(0).assume_init_read() };

        while let [prev, curr, ..] = items {
            // Moves popped element to end without using unsafe
            // MaybeUninit functions.
            ::core::mem::swap(prev, curr);

            // SAFETY: if items contains at least 2 elements it may always be split
            // at index 1.
            (_, items) = unsafe { items.split_at_mut_unchecked(1) };
        }

        popped
    }

    /// Pop the given element. Elemet is searched for
    /// in order of last element added first.
    pub fn pop<E>(&mut self, elem: &E) -> Option<T>
    where
        T: PartialEq<E>,
    {
        self.pop_by(|t| t.eq(elem))
    }

    /// Pop the element for which the given function returns true.
    /// Search is done in the order of last element added first.
    pub fn pop_by<F>(&mut self, cond: F) -> Option<T>
    where
        F: for<'a> FnMut(&'a T) -> bool,
    {
        let idx = self.index_of(cond)?;
        // SAFETY: Indices returned by `index_of` always exist.
        let (_, items) = unsafe { self.as_mut_maybeuninit_slice().split_at_mut_unchecked(idx) };

        // SAFETY: if an element was found by `index_of` size is greater than 0.
        let popped = unsafe { Self::pop_shift(items) };

        // SAFETY: If an element was found by `index_of` size must be greater than 0.
        self.size = unsafe { self.size.unchecked_sub(1) };

        Some(popped)
    }

    /// Write element to tail position.
    ///
    /// # Safety
    /// Size must be lower than N.
    unsafe fn write_tail(&mut self, elem: T) {
        // SAFETY: if size is lower than N
        // it is the index of a valid spot.
        let tail = unsafe { self.items.get_unchecked_mut(self.size) };
        tail.write(elem);

        // SAFETY: if size is lower than N
        // an add may not overflow.
        self.size = unsafe { self.size.unchecked_add(1) };
    }

    /// Push a value on the cache.
    ///
    /// If a prior entry was discarded it is returned.
    ///
    /// If N is 0, `elem` is returned.
    pub fn push(&mut self, elem: T) -> Option<T> {
        if N == 0 {
            Some(elem)
        } else if self.size == N {
            let items = self.as_mut_maybeuninit_slice();

            // SAFETY: if if-else reaches this point N is not 0.
            let popped = unsafe { Self::pop_shift(items) };

            // SAFETY: if N is not 0 and size is equal to N, size
            // must not be 0.
            self.size = unsafe { self.size.unchecked_sub(1) };

            // SAFETY: prior popping of an element ensures size is
            // lower than N.
            unsafe {
                self.write_tail(elem);
            }

            Some(popped)
        } else {
            // SAFETY: if size is not N it must be lower than N.
            unsafe { self.write_tail(elem) };
            None
        }
    }
}

#[cfg(test)]
mod test {
    //! Tests of cache queue.

    use crate::CacheQueue;

    #[test]
    fn push_pop_underfill() {
        let mut queue = CacheQueue::<10, _>::new();
        let n = 5;

        assert_eq!(queue.len(), 0);

        for i in 0..n {
            assert_eq!(queue.push(i), None);
            assert_eq!(queue.len(), i + 1);
        }

        for i in 0..n {
            assert_eq!(queue.pop(&i), Some(i));
            assert_eq!(queue.len(), n - 1 - i);
        }
    }

    #[test]
    fn push_pop_exact() {
        let mut queue = CacheQueue::<10, _>::new();
        let n = 10;

        assert_eq!(queue.len(), 0);

        for i in 0..n {
            assert_eq!(queue.push(i), None);
            assert_eq!(queue.len(), i + 1);
        }

        for i in 0..n {
            assert_eq!(queue.pop(&i), Some(i));
            assert_eq!(queue.len(), n - 1 - i);
        }
    }

    #[test]
    fn push_pop_exact_boxed() {
        let mut queue = CacheQueue::<10, _>::new();
        let n = 10;

        assert_eq!(queue.len(), 0);

        for i in 0..n {
            assert_eq!(queue.push(Box::new(i)), None);
            assert_eq!(queue.len(), i + 1);
        }

        for i in 0..n {
            assert_eq!(queue.pop_by(|t| i.eq(t)), Some(Box::new(i)));
            assert_eq!(queue.len(), n - 1 - i);
        }
    }

    #[test]
    fn push_pop_overfill() {
        let mut queue = CacheQueue::<10, _>::new();

        assert_eq!(queue.len(), 0);

        for i in 0..10 {
            assert_eq!(queue.push(i), None);
            assert_eq!(queue.len(), i + 1);
        }

        for i in 10..12 {
            assert_eq!(queue.push(i), Some(i - 10));
            assert_eq!(queue.len(), 10);
        }

        for i in 0..2 {
            assert_eq!(queue.pop(&i), None);
            assert_eq!(queue.len(), 10);
        }

        for i in 2..12 {
            assert_eq!(queue.pop(&i), Some(i));
            assert_eq!(queue.len(), 11 - i);
        }

        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn push_pop_zerosize() {
        let mut queue = CacheQueue::<0, _>::new();

        assert_eq!(queue.len(), 0);

        for i in 0..32 {
            assert_eq!(queue.push(i), Some(i));
            assert_eq!(queue.len(), 0);
            assert_eq!(queue.pop(&i), None);
            assert_eq!(queue.len(), 0);
        }
    }
}
