//! Extension traits.

/// Extension trait for results.
pub trait ResultExt<T> {
    /// Split a homogenous result into two options where the first option is some with
    /// the value if the result was `Ok` and the second if the result was `Err`.
    fn split_result(self) -> [Option<T>; 2];

    /// Map either variant.
    fn map_either<F, U>(self, f: F) -> Result<U, U>
    where
        F: FnOnce(T) -> U;
}

impl<T> ResultExt<T> for Result<T, T> {
    fn split_result(self) -> [Option<T>; 2] {
        match self {
            Ok(value) => [Some(value), None],
            Err(value) => [None, Some(value)],
        }
    }

    fn map_either<F, U>(self, f: F) -> Result<U, U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Ok(value) => Ok(f(value)),
            Err(value) => Err(f(value)),
        }
    }
}

/// Extension trait for booleans.
pub trait BoolExt {
    /// Convert to a result.
    ///
    /// # Succeeds
    /// If self is true.
    ///
    /// # Errors
    /// If self is false.
    fn to_result(self) -> Result<(), ()>;
}

impl BoolExt for bool {
    #[inline]
    fn to_result(self) -> Result<(), ()> {
        if self { Ok(()) } else { Err(()) }
    }
}
