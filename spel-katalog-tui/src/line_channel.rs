//! Channel to send lines.

use ::std::{
    io::{ErrorKind, Write},
    mem,
    num::NonZero,
    sync::mpsc::{Receiver, RecvError, Sender, TryRecvError, channel},
};

use ::derive_more::{AsMut, AsRef, Deref, DerefMut, IntoIterator};
use ::derive_new::new;

use crate::{ansi_cleanup::clean_bytes, bytes_to_string};

/// NonZero value of 1.
const ONE_NZ: NonZero<usize> = NonZero::new(1).unwrap();

/// Create a new line channel.
pub fn line_channel() -> (ChannelWriter, LineReceiver) {
    let (tx, rx) = channel();
    (ChannelWriter::new(tx), LineReceiver::new(rx))
}

/// Writer sending lines as completed in buffer.
#[derive(Debug, new)]
pub struct ChannelWriter {
    /// Sender to send lines to.
    tx: Sender<Vec<u8>>,
    /// Buffer for incomplete lines.
    #[new(default)]
    buf: Vec<u8>,
}

impl ChannelWriter {
    /// Extend internal buffer with bytes.
    fn extend_buf(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }
}

impl Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match ::memchr::memchr(b'\n', buf).and_then(|idx| buf.split_at_checked(idx)) {
            Some((bytes, [_n, ..])) => {
                self.extend_buf(bytes);
                self.tx
                    .send(mem::take(&mut self.buf))
                    .map_err(|_| ErrorKind::BrokenPipe)?;
                Ok(bytes.len() + 1)
            }
            None | Some((_, [])) => {
                self.extend_buf(buf);
                Ok(buf.len())
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Receiver for receiving, converting to printable, deduplicating
/// and storing lines.
#[derive(Debug, IntoIterator, Deref, AsRef, AsMut, DerefMut, new)]
pub struct LineReceiver {
    /// Receiver for getting lines.
    rx: Receiver<Vec<u8>>,
    #[new(default)]
    #[as_ref]
    #[as_mut]
    #[deref_mut]
    #[deref]
    #[into_iterator(owned, ref, ref_mut)]
    /// Lines received up to this point.
    pub lines: Vec<(NonZero<usize>, String)>,
}

impl LineReceiver {
    /// Add a line to internal state.
    fn add_line(&mut self, mut line: Vec<u8>) {
        clean_bytes(&mut line);
        let line = bytes_to_string(line);
        match self.last_mut() {
            Some((count, last_line)) if line == *last_line => {
                *count = count.saturating_add(1);
            }
            None | Some((_, _)) => {
                self.lines.push((ONE_NZ, line));
            }
        };
    }

    /// Receive linew until blocking is required to receive more.
    fn try_recv_loop(&mut self, mut init: NonZero<usize>, limit: usize) -> NonZero<usize> {
        for _ in 0..limit {
            if self.try_recv().is_err() {
                break;
            }
            init = init.saturating_add(1);
        }
        init
    }

    /// Receive all lines. Will block until channel is disconnected.
    pub fn recv_all(&mut self) {
        while self.recv().is_ok() {}
    }

    /// Receive a line from channel.
    ///
    /// # Errors
    /// If a line cannot be received.
    pub fn recv(&mut self) -> Result<(), RecvError> {
        let line = self.rx.recv()?;
        self.add_line(line);
        Ok(())
    }

    /// Try to receive a line from channel.
    ///
    /// # Errors
    /// If a line cannot be received. Or is not available.
    pub fn try_recv(&mut self) -> Result<(), TryRecvError> {
        let line = self.rx.try_recv()?;
        self.add_line(line);
        Ok(())
    }

    /// Receive multiple lines, with only the first receive blocking.
    /// Then continue untile empty, or disconnected.
    ///
    /// Will return ok as long as the first line was received.
    /// No indication as to if it stopped due to disconnection or
    /// emptyness after is given.
    ///
    /// # Errors
    /// If sender is closed.
    pub fn recv_many(&mut self, limit: usize) -> Result<NonZero<usize>, RecvError> {
        self.recv()?;
        Ok(self.try_recv_loop(ONE_NZ, limit))
    }

    /// Receive multiple lines until empty or disconnected, with no
    /// receive being blocking.
    ///
    /// Will return ok as long as the first line was received.
    /// No indication as to if it stopped due to disconnection or
    /// emptyness after is given.
    ///
    /// # Errors
    /// If sender is closed, or if no line was received.
    pub fn try_recv_many(&mut self, limit: usize) -> Result<NonZero<usize>, TryRecvError> {
        self.try_recv()?;
        Ok(self.try_recv_loop(ONE_NZ, limit))
    }
}
