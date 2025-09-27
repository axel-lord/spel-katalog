//! Terminal utilities.

use ::std::{
    io::{ErrorKind, PipeReader, Write, pipe},
    process::Stdio,
    sync::mpsc::{Receiver, Sender, channel},
};

use ::memchr::memchr;

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

/// Type building output sinks.
#[derive(Debug, Clone)]
pub enum SinkBuilder {
    /// Set sink to inherit parent.
    Inherit,
    /// Create and send a pipe.
    CreatePipe(Sender<(PipeReader, SinkIdentity)>),
}

impl SinkBuilder {
    /// Get a process sink.
    pub fn build(&self, id: impl FnOnce() -> SinkIdentity) -> ::std::io::Result<Stdio> {
        match self {
            SinkBuilder::Inherit => Ok(Stdio::inherit()),
            SinkBuilder::CreatePipe(sender) => {
                let (r, w) = pipe()?;

                sender
                    .send((r, id()))
                    .map_err(|err| ::std::io::Error::other(err.to_string()))?;

                Ok(Stdio::from(w))
            }
        }
    }

    /// Get two process sinks, which either both inherit parent or point to the same output.
    pub fn build_double(&self, id: impl FnOnce() -> SinkIdentity) -> ::std::io::Result<[Stdio; 2]> {
        match self {
            SinkBuilder::Inherit => Ok([Stdio::inherit(), Stdio::inherit()]),
            SinkBuilder::CreatePipe(sender) => {
                let (r, w) = pipe()?;
                let w2 = w.try_clone()?;

                sender
                    .send((r, id()))
                    .map_err(|err| ::std::io::Error::other(err.to_string()))?;

                Ok([w, w2].map(Stdio::from))
            }
        }
    }
}

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
