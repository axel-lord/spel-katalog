//! [SinkBuilder] impl.

use ::std::{
    io::{PipeReader, pipe},
    process::Stdio,
    sync::mpsc::Sender,
};

use crate::SinkIdentity;

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
