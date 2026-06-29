//! Crate with functionality for shared output sinks.

use ::std::{
    io::{PipeReader, PipeWriter, Stderr, Stdout, Write, pipe, stderr, stdout},
    os::fd::{AsFd, BorrowedFd},
    process::Stdio,
    sync::Arc,
};

use ::derive_more::Display;

/// [Write] implementation available for inherit and pipe
/// sinks.
#[derive(Debug)]
pub enum SinkWriter {
    /// Stdout writer.
    Stdout(Stdout),
    /// Stderr writer
    Stderr(Stderr),
    /// Pipe writer.
    Pipe(PipeWriter),
}

impl AsFd for SinkWriter {
    fn as_fd(&self) -> BorrowedFd<'_> {
        match self {
            SinkWriter::Stdout(stdout) => stdout.as_fd(),
            SinkWriter::Stderr(stderr) => stderr.as_fd(),
            SinkWriter::Pipe(pipe_writer) => pipe_writer.as_fd(),
        }
    }
}

impl SinkWriter {
    /// Get new writer of stdout variant.
    pub fn stdout() -> Self {
        Self::Stdout(stdout())
    }

    /// Get new writer of stderr variant.
    pub fn stderr() -> Self {
        Self::Stderr(stderr())
    }
}

impl Write for SinkWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            SinkWriter::Stdout(stdout) => stdout.write(buf),
            SinkWriter::Stderr(stderr) => stderr.write(buf),
            SinkWriter::Pipe(pipe_writer) => pipe_writer.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            SinkWriter::Stdout(stdout) => stdout.flush(),
            SinkWriter::Stderr(stderr) => stderr.flush(),
            SinkWriter::Pipe(pipe_writer) => pipe_writer.flush(),
        }
    }
}

/// The identity of a sink.
/// Used when choosing output.
#[derive(Debug, Clone, Display, Hash, PartialEq, Eq, PartialOrd, Ord)]
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
    CreatePipe(::flume::Sender<(PipeReader, SinkIdentity)>),
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
    ///
    /// # Errors
    /// If pipe creation fails.
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
    ///
    /// # Errors
    /// If pipe creation fails.
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

    /// Get two pipew riters if possible, returns `Ok(None)` if `Inherit`,
    /// otherwise attempts to clone/create a pipe. If creating `id` will
    /// be called to set identity of pipe.
    ///
    /// If `CreatePipe` and two pipes cannot be created no reader is sent.
    ///
    /// # Errors
    /// If pipe creation fails.
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

    /// Get two sink writers if possible. If `Inherit` will return [Stdout] and [Stderr] handles,
    /// if `CreatePipe` will create two pipes, and if `ClonePipe` will clone pipe twice.
    ///
    /// # Errors
    /// If pipe creation fails.
    pub fn get_writer_double(
        &self,
        id: impl FnOnce() -> SinkIdentity,
    ) -> ::std::io::Result<[SinkWriter; 2]> {
        match self {
            SinkBuilder::Inherit => Ok([SinkWriter::stdout(), SinkWriter::stderr()]),
            SinkBuilder::CreatePipe(sender) => {
                let (r, w) = pipe()?;
                let w2 = w.try_clone()?;

                sender
                    .send((r, id()))
                    .map_err(|err| ::std::io::Error::other(err.to_string()))?;

                Ok([SinkWriter::Pipe(w), SinkWriter::Pipe(w2)])
            }
            SinkBuilder::ClonePipe(pipe_writer) => {
                Ok([pipe_writer.try_clone()?, pipe_writer.try_clone()?].map(SinkWriter::Pipe))
            }
        }
    }

    /// Get a process sink.
    ///
    /// # Errors
    /// If pipes should be created but fails.
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
    ///
    /// # Errors
    /// If pipes should be created but fail.
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
