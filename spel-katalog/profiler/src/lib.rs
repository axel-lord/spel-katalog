//! Profiling utilities.

/// Timing provider.
#[doc(hidden)]
pub trait Timing {
    /// Instant used.
    type Instant;
    /// Duration used.
    type Duration;
    /// Collection used.
    type Collection: Default;

    /// Initialize timing.
    fn now() -> Self::Instant;

    /// Get time since instant.
    fn elapsed(since: Self::Instant) -> Self::Duration;

    /// Push duration to collection.
    fn push(collection: &mut Self::Collection, duration: Self::Duration);

    /// Get average duration from collection.
    fn average(collection: &Self::Collection) -> Self::Duration;

    /// If duration is compatible with [::core::time::Duration]
    /// run given function with it.
    fn with_duration(duration: Self::Duration, f: impl FnOnce(::core::time::Duration));

    /// If duration is compatible with [::core::time::Duration]
    /// run given function with all durations of collection.
    fn with_durations(collection: &Self::Collection, f: impl FnOnce(&[::core::time::Duration]));
}

cfg_select! {
    feature = "std_time" => {
        /// Default timing in use.
        #[doc(hidden)]
        pub type DefaultTiming = StdTime;
    }
    _ => {
        /// Default timing in use.
        #[doc(hidden)]
        pub type DefaultTiming = Noop;
    }
}

/// Instant used.
pub type Instant = <DefaultTiming as Timing>::Instant;
/// Duration used.
pub type Duration = <DefaultTiming as Timing>::Duration;
/// Collection used.
pub type Collection = <DefaultTiming as Timing>::Collection;

/// Initialize timing.
pub fn now() -> Instant {
    DefaultTiming::now()
}

/// Get time since instant.
pub fn elapsed(since: Instant) -> Duration {
    DefaultTiming::elapsed(since)
}

/// Push duration to collection.
pub fn push(collection: &mut Collection, duration: Duration) {
    DefaultTiming::push(collection, duration);
}

/// Get average duration from collection.
pub fn average(collection: &Collection) -> Duration {
    DefaultTiming::average(collection)
}

/// If duration is compatible with [::core::time::Duration]
/// run given function with it.
pub fn with_duration(duration: Duration, f: impl FnOnce(::core::time::Duration)) {
    DefaultTiming::with_duration(duration, f);
}

/// If duration is compatible with [::core::time::Duration]
/// run given function with all durations of collection.
pub fn with_durations(collection: &Collection, f: impl FnOnce(&[::core::time::Duration])) {
    DefaultTiming::with_durations(collection, f);
}

/// Time using standard library.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub enum StdTime {}

impl Timing for StdTime {
    type Instant = ::std::time::Instant;

    type Duration = ::core::time::Duration;

    type Collection = Vec<::core::time::Duration>;

    fn now() -> Self::Instant {
        ::std::time::Instant::now()
    }

    fn elapsed(since: Self::Instant) -> Self::Duration {
        since.elapsed()
    }

    fn push(collection: &mut Self::Collection, duration: Self::Duration) {
        collection.push(duration);
    }

    fn average(collection: &Self::Collection) -> Self::Duration {
        let len = collection.len() as f64;
        collection
            .iter()
            .copied()
            .reduce(|a, b| a.div_f64(len) + b.div_f64(len))
            .unwrap_or_default()
    }

    fn with_duration(duration: Self::Duration, f: impl FnOnce(::core::time::Duration)) {
        f(duration)
    }

    fn with_durations(collection: &Self::Collection, f: impl FnOnce(&[::core::time::Duration])) {
        f(collection)
    }
}

/// Do not time.
#[derive(Debug, Clone, Copy, Default)]
#[doc(hidden)]
pub struct Noop;

impl Timing for Noop {
    type Instant = Self;

    type Duration = Self;

    type Collection = Self;

    #[inline(always)]
    fn now() -> Self::Instant {
        Self
    }

    #[inline(always)]
    fn elapsed(_since: Self::Instant) -> Self::Duration {
        Self
    }

    #[inline(always)]
    fn push(_collection: &mut Self::Collection, _duration: Self::Duration) {}

    #[inline(always)]
    fn average(_collection: &Self::Collection) -> Self::Duration {
        Self
    }

    #[inline(always)]
    fn with_duration(_duration: Self::Duration, _f: impl FnOnce(::core::time::Duration)) {}

    #[inline(always)]
    fn with_durations(_collection: &Self::Collection, _f: impl FnOnce(&[::core::time::Duration])) {}
}
