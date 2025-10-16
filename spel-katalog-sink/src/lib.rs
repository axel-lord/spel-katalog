//! Crate with functionality for shared output sinks.

use ::std::{
    io::{PipeReader, PipeWriter, pipe},
    process::Stdio,
    sync::{Arc, mpsc::Sender},
};

use ::derive_more::Display;

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

/// Type building output sinks.
#[derive(Debug, Clone)]
pub enum SinkBuilder {
    /// Set sink to inherit parent.
    Inherit,
    /// Create and send a pipe.
    CreatePipe(Sender<(PipeReader, SinkIdentity)>),
    /// Clone an already created pipe.
    ClonePipe(Arc<PipeWriter>),
}

impl SinkBuilder {
    /// Create a pipe ock the builder's ability to create more pipes.
    /// Behaves differenctly depending on current state.
    ///
    /// If `CreatePipe` convert to `ClonePipe` with a newly created
    /// pipe using the given identity.
    ///
    /// If `ClonePipe`, clone self.
    ///
    /// If `Inherit`, create a new `Inherit` builder.
    pub fn with_locked_channel(
        &self,
        id: impl FnOnce() -> SinkIdentity,
    ) -> ::std::io::Result<Self> {
        Ok(match self {
            SinkBuilder::Inherit => Self::Inherit,
            SinkBuilder::CreatePipe(sender) => {
                let (r, w) = pipe()?;

                sender
                    .send((r, id()))
                    .map_err(|err| ::std::io::Error::other(err.to_string()))?;

                Self::ClonePipe(Arc::new(w))
            }
            SinkBuilder::ClonePipe(pipe_writer) => Self::ClonePipe(pipe_writer.clone()),
        })
    }

    /// Get a pipewriter if possible, returns `Ok(None)` if `Inherit`,
    /// otherwise attempts to clone/create a pipe. If creating `id` will
    /// be called to set identity of pipe.
    pub fn get_pipe_writer(
        &self,
        id: impl FnOnce() -> SinkIdentity,
    ) -> ::std::io::Result<Option<PipeWriter>> {
        match self {
            SinkBuilder::Inherit => Ok(None),
            SinkBuilder::CreatePipe(sender) => {
                let (r, w) = pipe()?;

                sender
                    .send((r, id()))
                    .map_err(|err| ::std::io::Error::other(err.to_string()))?;

                Ok(Some(w))
            }
            SinkBuilder::ClonePipe(pipe_writer) => pipe_writer.try_clone().map(Some),
        }
    }

    /// Get two pipewriters if possible, returns `Ok(None)` if `Inherit`,
    /// otherwise attempts to clone/create a pipe. If creating `id` will
    /// be called to set identity of pipe.
    ///
    /// If `CreatePipe` and two pipes cannot be created no reader is sent.
    pub fn get_pipe_writer_double(
        &self,
        id: impl FnOnce() -> SinkIdentity,
    ) -> ::std::io::Result<Option<[PipeWriter; 2]>> {
        match self {
            SinkBuilder::Inherit => Ok(None),
            SinkBuilder::CreatePipe(sender) => {
                let (r, w) = pipe()?;
                let w2 = w.try_clone()?;

                sender
                    .send((r, id()))
                    .map_err(|err| ::std::io::Error::other(err.to_string()))?;

                Ok(Some([w, w2]))
            }
            SinkBuilder::ClonePipe(pipe_writer) => {
                Ok(Some([pipe_writer.try_clone()?, pipe_writer.try_clone()?]))
            }
        }
    }

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
            SinkBuilder::ClonePipe(pipe_writer) => pipe_writer.try_clone().map(Stdio::from),
        }
    }

    /// Get two process sinks, which either both inherit parent or point to the same output.
    ///
    /// If `CreatePipe` and two pipes cannot be created no reader is sent.
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
            SinkBuilder::ClonePipe(pipe_writer) => Ok([
                pipe_writer.try_clone()?.into(),
                pipe_writer.try_clone()?.into(),
            ]),
        }
    }
}
