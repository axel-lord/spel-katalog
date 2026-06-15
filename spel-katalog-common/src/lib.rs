//! Common types for communication across crates.

mod or_request;
mod status_sender;
mod display_bytes {
    //! Display wrapper for bytes.

    use ::core::{
        fmt::{Debug, Display},
        hash::Hash,
        ops::Deref,
    };

    /// Create a display adapter for a byte slice.
    pub fn display_bytes(
        bytes: &[u8],
    ) -> impl '_ + Display + Debug + Copy + Ord + Hash + Deref<Target = [u8]> {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(transparent)]
        struct Disp<'a>(&'a [u8]);

        impl Debug for Disp<'_> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str("b\"")?;

                for chunk in self.0.utf8_chunks() {
                    for chr in chunk.valid().chars() {
                        write!(f, "{}", chr.escape_debug())?;
                    }

                    for b in chunk.invalid() {
                        write!(f, "\\x{b:02X}")?;
                    }
                }

                f.write_str("\"")
            }
        }

        impl Display for Disp<'_> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                for chunk in self.0.utf8_chunks() {
                    f.write_str(chunk.valid())?;

                    for b in chunk.invalid() {
                        write!(f, "\\x{b:02X}")?;
                    }
                }
                Ok(())
            }
        }

        impl Deref for Disp<'_> {
            type Target = [u8];

            fn deref(&self) -> &Self::Target {
                self.0
            }
        }

        Disp(bytes)
    }
}

pub use self::{
    display_bytes::display_bytes,
    or_request::{IntoOrRequest, OrRequest},
    status_sender::StatusSender,
};

pub mod in_place;
pub mod lazy;
pub mod styling;
pub mod w;

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
