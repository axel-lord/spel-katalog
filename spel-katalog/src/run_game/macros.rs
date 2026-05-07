/// Convert inputs to an array of [OsString]
///
/// ```
/// assert_eq!(
///     args!["echo", "Hello", "World!"],
///     [OsString::from("echo"), OsString::from("Hello"), OsString::from("World!")]
/// );
/// ```
macro_rules! args {
    ($($arg:expr),* $(,)?) => {
        [$(::std::ffi::OsString::from($arg)),*]
    };
}

/// Create a formatted string error.
/// ```
/// let err = strerror!("Error occured, {}", 15);
/// ```
macro_rules! strerror {
    ($($arg:tt)*) => {
        $crate::run_game::strerror::StrError::fmt(format_args!($($arg)*))
    };
}

pub(crate) use args;
pub(crate) use strerror;
