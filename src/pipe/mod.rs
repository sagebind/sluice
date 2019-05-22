//! Asynchronous in-memory byte buffers aimed at producer-consumer problems.
//!
//! Pipes are like byte-oriented channels that implement I/O traits for reading
//! and writing.

use futures::prelude::*;
use std::io;
use std::pin::Pin;
use std::task::*;

mod chunked;

/// How many chunks should be available in a chunked pipe. Default is 4, which
/// strikes a good balance of low memory usage and throughput.
const DEFAULT_CHUNK_COUNT: usize = 4;

/// Creates a new asynchronous pipe with the default configuration.
///
/// The default implementation guarantees that when writing a slice of bytes,
/// either the entire slice is written at once or not at all. Slices will never
/// be partially written.
pub fn pipe() -> (PipeReader, PipeWriter) {
    let (reader, writer) = chunked::new(DEFAULT_CHUNK_COUNT);

    (
        PipeReader {
            inner: reader,
        },
        PipeWriter {
            inner: writer,
        },
    )
}

/// The reading end of an asynchronous pipe.
pub struct PipeReader {
    inner: chunked::Reader,
}

impl AsyncRead for PipeReader {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

/// The writing end of an asynchronous pipe.
pub struct PipeWriter {
    inner: chunked::Writer,
}

impl AsyncWrite for PipeWriter {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_close(cx)
    }
}
