//! Failure macro.

macro_rules! failure {
($panic:expr, $($arg:tt)+) => {
    if $panic {
        ::log::error!($($arg)*);
        crate::dependency::DependencyResult::Panic
    } else {
        ::log::info!($($arg)*);
        crate::dependency::DependencyResult::Failure
    }
};
}
pub(crate) use failure;
