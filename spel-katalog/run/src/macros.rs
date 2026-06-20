//! Macros in use to create values.

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

pub(crate) use args;
