//! [SinkIdentity] impl.

use ::derive_more::Display;

/// The identity of a sink.
/// Used when choosing output.
#[derive(Debug, Clone, Display, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum SinkIdentity {
    /// A Static string.
    StaticName(&'static str),
    /// A normal string.
    Name(String),
    /// Process id.
    #[display("Process({_0})")]
    ProcessId(i64),
    /// Game Id
    #[display("Game({_0})")]
    GameId(i64),
}

impl From<String> for SinkIdentity {
    fn from(value: String) -> Self {
        SinkIdentity::Name(value)
    }
}

impl From<&'static str> for SinkIdentity {
    fn from(value: &'static str) -> Self {
        SinkIdentity::StaticName(value)
    }
}

/// Trait used to supply factories for sink identity creation.
pub trait SinkIdentityFactory {
    /// Provide sink identity.
    fn sink_identity(self) -> SinkIdentity;
}

impl<F, T> SinkIdentityFactory for F
where
    F: FnOnce() -> T,
    T: Into<SinkIdentity>,
{
    fn sink_identity(self) -> SinkIdentity {
        (self)().into()
    }
}
