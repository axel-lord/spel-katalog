//! [OrStatus] impl.

use ::derive_more::IsVariant;

/// A Message or a status string.
#[derive(Debug, Clone, IsVariant)]
pub enum OrStatus<M> {
    /// This value is a message.
    Message(M),
    /// This value is a status message.
    Status(String),
}

impl<T> OrStatus<T> {
    /// Create a new instance from a message.
    pub fn new(value: T) -> Self {
        Self::Message(value)
    }

    /// Convert between message types.
    pub fn convert<V>(self) -> OrStatus<V>
    where
        V: From<T>,
    {
        match self {
            OrStatus::Message(value) => OrStatus::Message(value.into()),
            OrStatus::Status(status) => OrStatus::Status(status),
        }
    }
}

impl<T> From<String> for OrStatus<T> {
    fn from(value: String) -> Self {
        Self::Status(value)
    }
}
