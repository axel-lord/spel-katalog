//! Terminal utilities.

mod line_pipe;
mod sink_builder;
mod tui;

use ::std::{fmt::Debug, io::PipeReader, sync::mpsc::Receiver};

use ::derive_more::Display;

pub use self::{line_pipe::LinePipe, sink_builder::SinkBuilder};

/// The identity of a sink.
/// Used when choosing output.
#[derive(Debug, Clone, Display)]
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

/// Collection of channels in use by tui.
pub struct Channels {
    /// Oneshot channel to shut down gui.
    pub exit_tx: Box<dyn FnOnce()>,
    /// Receive a new pipe.
    pub pipe_rx: Receiver<(PipeReader, SinkIdentity)>,
    /// Receive log messages.
    pub log_rx: Receiver<Vec<u8>>,
}

impl Debug for Channels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channels")
            .field("exit_tx", &"oneshot")
            .field("pipe_rx", &self.pipe_rx)
            .field("log_rx", &self.log_rx)
            .finish()
    }
}

/// Run tui.
pub fn tui(channels: Channels) -> ::std::io::Result<()> {
    let mut terminal = ::ratatui::init();
    let res = tui::tui(channels, &mut terminal);
    ::ratatui::restore();
    res
}
