//! Terminal utilities.

use ::std::{
    io::{ErrorKind, Write},
    sync::mpsc::{Receiver, Sender, channel},
};

use ::memchr::memchr;

/// Sender implementing write sending on line breaks.
#[derive(Debug)]
pub struct LinePipe {
    sender: Sender<Vec<u8>>,
    buffer: Vec<u8>,
}

impl LinePipe {
    /// Create a channel.
    pub fn channel() -> (Self, Receiver<Vec<u8>>) {
        let (sender, receiver) = channel();
        (
            Self {
                sender,
                buffer: Vec::new(),
            },
            receiver,
        )
    }
}

impl Write for LinePipe {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(idx) = memchr(b'\n', buf) {
            self.buffer.extend_from_slice(&buf[..idx]);

            if self
                .sender
                .send(::std::mem::take(&mut self.buffer))
                .is_err()
            {
                return Err(ErrorKind::BrokenPipe.into());
            }

            Ok(idx + 1)
        } else {
            self.buffer.extend_from_slice(buf);
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
