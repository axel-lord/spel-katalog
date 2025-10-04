//! Terminal utilities.

use ::std::{fmt::Debug, io::PipeReader, sync::mpsc::Receiver};

use ::derive_more::Display;

pub use self::{
    line_channel::{ChannelWriter, LineReceiver, line_channel},
    sink_builder::SinkBuilder,
};

mod ansi_cleanup;
mod line_channel;
mod sink_builder;
mod tui;

mod log_channel {}

fn bytes_to_string(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(err) => {
            use ::std::fmt::Write;
            let bytes = err.as_bytes();
            let mut buf = String::with_capacity(bytes.len());
            for chunk in err.as_bytes().utf8_chunks() {
                buf.push_str(chunk.valid());
                for byte in chunk.invalid() {
                    write!(buf, "\\x{:02X}", byte).expect("write to String should succeed");
                }
            }
            buf
        }
    }
}

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
    pub log_rx: LineReceiver,
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
    ::ratatui::crossterm::execute!(
        ::std::io::stdout(),
        ::ratatui::crossterm::event::EnableMouseCapture
    )?;
    let mut terminal = ::ratatui::init();
    let res = tui::tui(channels, &mut terminal);
    ::ratatui::restore();
    _ = ::ratatui::crossterm::execute!(
        ::std::io::stdout(),
        ::ratatui::crossterm::event::DisableMouseCapture
    );
    res
}
