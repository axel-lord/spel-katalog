//! Terminal utilities.

mod line_pipe;
mod sink_builder;

pub use self::{line_pipe::LinePipe, sink_builder::SinkBuilder};

/// The identity of a sink.
/// Used when choosing output.
#[derive(Debug, Clone)]
pub enum SinkIdentity {
    /// A Static string.
    StaticName(&'static str),
    /// A normal string.
    Name(String),
    /// Process id.
    ProcessId(i64),
    /// Game Id
    GameId(i64),
}
