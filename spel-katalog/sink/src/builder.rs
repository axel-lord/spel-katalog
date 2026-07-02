//! [SinkBuilder] impl.

use ::std::{
    io::{PipeReader, PipeWriter, pipe},
    process::Stdio,
    sync::Arc,
};

use crate::{AsyncSinkWriter, SinkIdentity, SinkIdentityFactory, SinkWriter};

/// Type building output sinks.
#[derive(Debug, Clone)]
pub enum SinkBuilder {
    /// Sink builder produces writers writing to [stdout][::std::io::Stdout] and [stderr][::std::io::Stderr].
    Inherit,
    /// Sink produces new pipes with the read end sent through the channel.
    CreatePipe(::flume::Sender<(PipeReader, SinkIdentity)>),
    /// Sink builder clones the given writer using [try_clone][PipeWriter::try_clone].
    ClonePipe(Arc<PipeWriter>),
    /// Sink builder clones the given writers using [try_clone][PipeWriter::try_clone].
    ClonePipes {
        /// Pipe cloned for stdout.
        stdout: Arc<PipeWriter>,
        /// Pipe cloned for stderr.
        stderr: Arc<PipeWriter>,
    },
}

impl From<PipeWriter> for SinkBuilder {
    fn from(value: PipeWriter) -> Self {
        SinkBuilder::ClonePipe(Arc::new(value))
    }
}

impl From<(PipeWriter, PipeWriter)> for SinkBuilder {
    fn from((stdout, stderr): (PipeWriter, PipeWriter)) -> Self {
        SinkBuilder::ClonePipes {
            stdout: Arc::new(stdout),
            stderr: Arc::new(stderr),
        }
    }
}

impl From<[PipeWriter; 2]> for SinkBuilder {
    fn from([stdout, stderr]: [PipeWriter; 2]) -> Self {
        SinkBuilder::ClonePipes {
            stdout: Arc::new(stdout),
            stderr: Arc::new(stderr),
        }
    }
}

impl From<::flume::Sender<(PipeReader, SinkIdentity)>> for SinkBuilder {
    fn from(value: ::flume::Sender<(PipeReader, SinkIdentity)>) -> Self {
        SinkBuilder::CreatePipe(value)
    }
}

impl SinkBuilder {
    /// Create a pipe ock the builder's ability to create more pipes.
    /// Behaves differenctly depending on current state.
    ///
    /// If `CreatePipe` convert to `ClonePipe` with a newly created
    /// pipe using the given identity.
    ///
    /// If `ClonePipe` or `ClonePipes`, clone self.
    ///
    /// If `Inherit`, create a new `Inherit` builder.
    ///
    /// # Errors
    /// If pipe creation fails.
    pub fn with_locked_channel(&self, id: impl SinkIdentityFactory) -> ::std::io::Result<Self> {
        Ok(match self {
            SinkBuilder::Inherit => Self::Inherit,
            SinkBuilder::CreatePipe(sender) => {
                let (r, w) = pipe()?;

                sender
                    .send((r, id.sink_identity()))
                    .map_err(|err| ::std::io::Error::other(err.to_string()))?;

                Self::ClonePipe(Arc::new(w))
            }
            SinkBuilder::ClonePipe(pipe_writer) => Self::ClonePipe(pipe_writer.clone()),
            SinkBuilder::ClonePipes { stdout, stderr } => Self::ClonePipes {
                stdout: stdout.clone(),
                stderr: stderr.clone(),
            },
        })
    }

    /// Get two pipew riters if possible, returns `Ok(None)` if `Inherit`,
    /// otherwise attempts to clone/create a pipe. If creating `id` will
    /// be called to set identity of pipe.
    ///
    /// If `CreatePipe` and two pipes cannot be created no reader is sent.
    ///
    /// # Errors
    /// If pipe creation fails.
    pub fn pipe_writers(
        &self,
        id: impl SinkIdentityFactory,
    ) -> ::std::io::Result<Option<[PipeWriter; 2]>> {
        match self {
            SinkBuilder::Inherit => Ok(None),
            SinkBuilder::CreatePipe(sender) => {
                let (r, w) = pipe()?;
                let w2 = w.try_clone()?;

                sender
                    .send((r, id.sink_identity()))
                    .map_err(|err| ::std::io::Error::other(err.to_string()))?;

                Ok(Some([w, w2]))
            }
            SinkBuilder::ClonePipe(pipe_writer) => {
                Ok(Some([pipe_writer.try_clone()?, pipe_writer.try_clone()?]))
            }

            SinkBuilder::ClonePipes { stdout, stderr } => {
                Ok(Some([stdout.try_clone()?, stderr.try_clone()?]))
            }
        }
    }

    /// Get two sink writers if possible. If `Inherit` will return
    /// [Stdout][::std::io::Stdout] and [Stderr][::std::io::Stderr] handles,
    /// if `CreatePipe` will create two pipes, and if `ClonePipe` or `ClonePipes` will clone pipe.
    ///
    /// # Errors
    /// If pipe creation fails.
    pub fn writers(&self, id: impl SinkIdentityFactory) -> ::std::io::Result<[SinkWriter; 2]> {
        match self {
            SinkBuilder::Inherit => Ok([SinkWriter::stdout(), SinkWriter::stderr()]),
            SinkBuilder::CreatePipe(sender) => {
                let (r, w) = pipe()?;
                let w2 = w.try_clone()?;

                sender
                    .send((r, id.sink_identity()))
                    .map_err(|err| ::std::io::Error::other(err.to_string()))?;

                Ok([SinkWriter::Pipe(w), SinkWriter::Pipe(w2)])
            }
            SinkBuilder::ClonePipe(pipe_writer) => {
                Ok([pipe_writer.try_clone()?, pipe_writer.try_clone()?].map(SinkWriter::Pipe))
            }
            SinkBuilder::ClonePipes { stdout, stderr } => {
                Ok([stdout.try_clone()?, stderr.try_clone()?].map(SinkWriter::Pipe))
            }
        }
    }

    /// Get two async sink writers if possible. If `Inherit` will return
    /// [Stdout][::std::io::Stdout] and [Stderr][::std::io::Stderr] handles,
    /// if `CreatePipe` will create two pipes, and if `ClonePipe` or `ClonePipes` will clone pipe/s.
    ///
    /// # Errors
    /// If pipe creation fails.
    pub fn async_writers(
        &self,
        id: impl SinkIdentityFactory,
    ) -> ::std::io::Result<[AsyncSinkWriter; 2]> {
        Ok(self.writers(id)?.map(AsyncSinkWriter::from))
    }

    /// Get two process sinks, which either both inherit parent or point to the same output.
    ///
    /// If `CreatePipe` and two pipes cannot be created no reader is sent.
    ///
    /// # Errors
    /// If pipes should be created but fail.
    pub fn build(&self, id: impl SinkIdentityFactory) -> ::std::io::Result<[Stdio; 2]> {
        match self {
            SinkBuilder::Inherit => Ok([Stdio::inherit(), Stdio::inherit()]),
            SinkBuilder::CreatePipe(sender) => {
                let (r, w) = pipe()?;
                let w2 = w.try_clone()?;

                sender
                    .send((r, id.sink_identity()))
                    .map_err(|err| ::std::io::Error::other(err.to_string()))?;

                Ok([w, w2].map(Stdio::from))
            }
            SinkBuilder::ClonePipe(pipe_writer) => {
                Ok([pipe_writer.try_clone()?, pipe_writer.try_clone()?].map(Stdio::from))
            }
            SinkBuilder::ClonePipes { stdout, stderr } => {
                Ok([stdout.try_clone()?, stderr.try_clone()?].map(Stdio::from))
            }
        }
    }
}
