//! Terminal utilities.

use ::std::{fmt::Debug, io::PipeReader, sync::mpsc::Receiver};

use ::spel_katalog_sink::SinkIdentity;

pub use self::line_channel::{ChannelWriter, LineReceiver, line_channel};

mod ansi_cleanup;
mod line_channel;
mod tui;

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
pub fn tui(channels: Channels, keep_terminal: bool) -> ::std::io::Result<()> {
    ::ratatui::crossterm::execute!(
        ::std::io::stdout(),
        ::ratatui::crossterm::event::EnableMouseCapture
    )?;
    let mut terminal = ::ratatui::init();
    let res = tui::tui(channels, &mut terminal, keep_terminal);
    ::ratatui::restore();
    _ = ::ratatui::crossterm::execute!(
        ::std::io::stdout(),
        ::ratatui::crossterm::event::DisableMouseCapture
    );
    res
}
