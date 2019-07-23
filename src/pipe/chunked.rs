//! Generally a ring buffer is an efficient and appropriate data structure for
//! asynchronously transmitting a stream of bytes between two threads that also
//! gives you control over memory allocation to avoid consuming an unknown
//! amount of memory. Setting a fixed memory limit also gives you a degree of
//! flow control if the producer ends up being faster than the consumer.
//!
//! But for some use cases a ring buffer will not work if an application uses
//! its own internal buffer management and requires you to consume either all of
//! a "chunk" of bytes, or none of it.
//!
//! Because of these constraints, instead we use a quite unique type of buffer
//! that uses a fixed number of growable buffers that are exchanged back and
//! forth between a producer and a consumer. Since each buffer is a vector, it
//! can grow to whatever size is required of it in order to fit a single chunk.
//!
//! To avoid the constant allocation overhead of creating a new buffer for every
//! chunk, after a consumer finishes reading from a buffer, it returns the
//! buffer to the producer over a channel to be reused. The number of buffers
//! available in this system is fixed at creation time, so the only allocations
//! that happen during reads and writes are occasional reallocation for each
//! individual vector to fit larger chunks of bytes that don't already fit.

use futures_channel::mpsc;
use futures_core::Stream;
use futures_io::{AsyncRead, AsyncWrite};
use std::io::{self, Cursor};
use std::pin::Pin;
use std::task::{Context, Poll};

/// Create a new chunked pipe with room for a fixed number of chunks.
///
/// The `count` parameter sets how many buffers are available in the pipe at
/// once. Smaller values will reduce the number of allocations and reallocations
/// may be required when writing and reduce overall memory usage. Larger values
/// reduce the amount of waiting done between chunks if you have a producer and
/// consumer that run at different speeds.
///
/// If `count` is set to 1, then the pipe is essentially serial, since only the
/// reader or writer can operate on the single buffer at one time and cannot be
/// run in parallel.
pub(crate) fn new(count: usize) -> (Reader, Writer) {
    let (mut buf_pool_tx, buf_pool_rx) = mpsc::channel(count);
    let (buf_stream_tx, buf_stream_rx) = mpsc::channel(count);

    // Fill up the buffer pool.
    for _ in 0..count {
        buf_pool_tx.try_send(Cursor::new(Vec::new())).expect("buffer pool overflow");
    }

    let reader = Reader {
        buf_pool_tx,
        buf_stream_rx,
        chunk: None,
    };

    let writer = Writer {
        buf_pool_rx,
        buf_stream_tx,
    };

    (reader, writer)
}

/// The reading half of a chunked pipe.
pub(crate) struct Reader {
    /// A channel of incoming chunks from the writer.
    buf_pool_tx: mpsc::Sender<Cursor<Vec<u8>>>,

    /// A channel of chunk buffers that have been consumed and can be reused.
    buf_stream_rx: mpsc::Receiver<Cursor<Vec<u8>>>,

    /// A chunk currently being read from.
    chunk: Option<Cursor<Vec<u8>>>,
}

impl AsyncRead for Reader {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        // Fetch the chunk to read from. If we already have one from a previous
        // read, use that, otherwise receive the next chunk from the writer.
        let mut chunk = match self.chunk.take() {
            Some(chunk) => chunk,

            None => match Pin::new(&mut self.buf_stream_rx).poll_next(cx) {
                // Wait for a new chunk to be delivered.
                Poll::Pending => return Poll::Pending,

                // Pipe has closed, so return EOF.
                Poll::Ready(None) => return Poll::Ready(Ok(0)),

                // Accept the new chunk.
                Poll::Ready(Some(buf)) => buf,
            }
        };

        // Do the read.
        let len = match Pin::new(&mut chunk).poll_read(cx, buf) {
            Poll::Pending => unreachable!(),
            Poll::Ready(Ok(len)) => len,
            Poll::Ready(Err(e)) => panic!("cursor returned an error: {}", e),
        };

        // If the chunk is not empty yet, keep it for a future read.
        if chunk.position() < chunk.get_ref().len() as u64 {
            self.chunk = Some(chunk);
        }

        // Otherwise, return it to the writer to be reused.
        else {
            chunk.set_position(0);
            chunk.get_mut().clear();

            match self.buf_pool_tx.try_send(chunk) {
                Ok(()) => {}

                Err(e) => {
                    // We pre-fill the buffer pool channel with an exact number
                    // of buffers, so this can never happen.
                    if e.is_full() {
                        panic!("buffer pool overflow")
                    }

                    // If the writer disconnects, then we'll just discard this
                    // buffer and any subsequent buffers until we've read
                    // everything still in the pipe.
                    else if e.is_disconnected() {
                        // Nothing!
                    }

                    // Some other error occurred.
                    else {
                        return Poll::Ready(Err(io::ErrorKind::BrokenPipe.into()));
                    }
                }
            }
        }

        Poll::Ready(Ok(len))
    }
}

/// Writing half of a chunked pipe.
pub(crate) struct Writer {
    /// A channel of chunks to send to the reader.
    buf_pool_rx: mpsc::Receiver<Cursor<Vec<u8>>>,

    /// A channel of incoming buffers to write chunks to.
    buf_stream_tx: mpsc::Sender<Cursor<Vec<u8>>>,
}

impl AsyncWrite for Writer {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        // If the pipe is closed then return prematurely, otherwise we'd be
        // spending time writing the entire buffer only to discover that it is
        // closed afterward.
        if self.buf_stream_tx.is_closed() {
            return Poll::Ready(Err(io::ErrorKind::BrokenPipe.into()));
        }

        // Attempt to grab an available buffer to write the chunk to.
        match Pin::new(&mut self.buf_pool_rx).poll_next(cx) {
            // Wait for the reader to finish reading a chunk.
            Poll::Pending => Poll::Pending,

            // Pipe has closed.
            Poll::Ready(None) => Poll::Ready(Err(io::ErrorKind::BrokenPipe.into())),

            // An available buffer has been found.
            Poll::Ready(Some(mut chunk)) => {
                // Write the buffer to the chunk.
                chunk.get_mut().extend_from_slice(buf);

                // Send the chunk to the reader.
                match self.buf_stream_tx.try_send(chunk) {
                    Ok(()) => Poll::Ready(Ok(buf.len())),

                    Err(e) => {
                        if e.is_full() {
                            panic!("buffer pool overflow")
                        } else {
                            Poll::Ready(Err(io::ErrorKind::BrokenPipe.into()))
                        }
                    }
                }
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.buf_stream_tx.close_channel();
        Poll::Ready(Ok(()))
    }
}

#[cfg(all(test, feature = "nightly"))]
mod tests {
    use futures::executor::block_on;
    use futures::prelude::*;
    use futures::task::noop_waker;
    use super::*;

    #[test]
    fn read_then_write() {
        block_on(async {
            let (mut reader, mut writer) = new(1);

            writer.write_all(b"hello").await.unwrap();

            let mut dest = [0; 5];
            assert_eq!(reader.read(&mut dest).await.unwrap(), 5);
            assert_eq!(&dest, b"hello");
        })
    }

    #[test]
    fn reader_still_drainable_after_writer_disconnects() {
        block_on(async {
            let (mut reader, mut writer) = new(1);

            writer.write_all(b"hello").await.unwrap();

            drop(writer);

            let mut dest = [0; 5];
            assert_eq!(reader.read(&mut dest).await.unwrap(), 5);
            assert_eq!(&dest, b"hello");

            assert_eq!(reader.read(&mut dest).await.unwrap(), 0);
        })
    }

    #[test]
    fn writer_errors_if_reader_is_dropped() {
        let waker = noop_waker();
        let mut context = Context::from_waker(&waker);

        let (reader, mut writer) = new(2);

        drop(reader);

        match writer.write(b"hello").poll_unpin(&mut context) {
            Poll::Ready(Err(e)) => assert_eq!(e.kind(), io::ErrorKind::BrokenPipe),
            _ => panic!("expected poll to be ready"),
        }
    }
}
