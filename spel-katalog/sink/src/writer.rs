//! [SinkWriter] impl.
use ::core::{
    pin::Pin,
    task::{self, Poll},
};
use ::std::{
    io::{self, PipeWriter, Stderr, Stdout, Write, stderr, stdout},
    os::fd::{AsFd, BorrowedFd, OwnedFd},
};

use ::pin_project::pin_project;
use ::smol::{Unblock, io::AsyncWrite};
use ::tap::{Conv, Pipe};

/// [Write] implementation available
/// inherit and pipe sinks.
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

    /// Convert into an async writer.
    pub fn into_async(self) -> AsyncSinkWriter {
        match self {
            SinkWriter::Stdout(stdout) => stdout.pipe(Unblock::new).pipe(AsyncSinkWriter::Stdout),
            SinkWriter::Stderr(stderr) => stderr.pipe(Unblock::new).pipe(AsyncSinkWriter::Stderr),
            SinkWriter::Pipe(pipe_writer) => pipe_writer
                .conv::<OwnedFd>()
                .conv::<::smol::fs::File>()
                .pipe(AsyncSinkWriter::Pipe),
        }
    }
}

impl Write for SinkWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            SinkWriter::Stdout(stdout) => stdout.write(buf),
            SinkWriter::Stderr(stderr) => stderr.write(buf),
            SinkWriter::Pipe(pipe_writer) => pipe_writer.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            SinkWriter::Stdout(stdout) => stdout.flush(),
            SinkWriter::Stderr(stderr) => stderr.flush(),
            SinkWriter::Pipe(pipe_writer) => pipe_writer.flush(),
        }
    }
}

/// [AsyncWrite] implementation available for
/// inherit and pipe sinks.
#[derive(Debug)]
#[pin_project(project = AsyncSinkWriterProjection)]
pub enum AsyncSinkWriter {
    /// Stdout writer.
    Stdout(#[pin] Unblock<Stdout>),
    /// Stderr writer
    Stderr(#[pin] Unblock<Stderr>),
    /// Pipe writer.
    Pipe(#[pin] ::smol::fs::File),
}

impl From<SinkWriter> for AsyncSinkWriter {
    #[inline]
    fn from(value: SinkWriter) -> Self {
        value.into_async()
    }
}

impl AsyncWrite for AsyncSinkWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.project() {
            AsyncSinkWriterProjection::Stdout(pin) => AsyncWrite::poll_write(pin, cx, buf),
            AsyncSinkWriterProjection::Stderr(pin) => AsyncWrite::poll_write(pin, cx, buf),
            AsyncSinkWriterProjection::Pipe(pin) => AsyncWrite::poll_write(pin, cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        match self.project() {
            AsyncSinkWriterProjection::Stdout(pin) => AsyncWrite::poll_flush(pin, cx),
            AsyncSinkWriterProjection::Stderr(pin) => AsyncWrite::poll_flush(pin, cx),
            AsyncSinkWriterProjection::Pipe(pin) => AsyncWrite::poll_flush(pin, cx),
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        match self.project() {
            AsyncSinkWriterProjection::Stdout(pin) => AsyncWrite::poll_close(pin, cx),
            AsyncSinkWriterProjection::Stderr(pin) => AsyncWrite::poll_close(pin, cx),
            AsyncSinkWriterProjection::Pipe(pin) => AsyncWrite::poll_close(pin, cx),
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        match self.project() {
            AsyncSinkWriterProjection::Stdout(pin) => {
                AsyncWrite::poll_write_vectored(pin, cx, bufs)
            }
            AsyncSinkWriterProjection::Stderr(pin) => {
                AsyncWrite::poll_write_vectored(pin, cx, bufs)
            }
            AsyncSinkWriterProjection::Pipe(pin) => AsyncWrite::poll_write_vectored(pin, cx, bufs),
        }
    }
}
