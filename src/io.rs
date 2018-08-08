//! Provides types specifically designed for working with bytes and I/O.

use buffers::{atomic, ReadableBuffer, WritableBuffer};
use std::io;
use std::sync::{Arc, Condvar, Mutex};

const DEFAULT_CAPACITY: usize = 8192;
const FLAG_NONBLOCKING: u8 = 0b0001;

/// Open a new pipe and return a reader and writer pair.
///
/// The pipe will be allocated with a default buffer size of 8 KiB. To customize the buffer size, use
/// [`PipeBuilder`](struct.PipeBuilder.html) instead.
pub fn pipe() -> (PipeReader, PipeWriter) {
    PipeBuilder::default().build()
}

/// Creates new pipes with configurable properties.
pub struct PipeBuilder {
    flags: u8,
    capacity: usize,
}

impl Default for PipeBuilder {
    fn default() -> Self {
        Self {
            flags: 0,
            capacity: DEFAULT_CAPACITY,
        }
    }
}

impl PipeBuilder {
    /// Enable or disable non-blocking behavior.
    ///
    /// If non-blocking mode is enabled, any reads or writes that cannot be completed until later will return an
    /// `WouldBlock` error instead of blocking the current thread.
    pub fn nonblocking(&mut self, nonblocking: bool) -> &mut Self {
        if nonblocking {
            self.flags |= FLAG_NONBLOCKING;
        } else {
            self.flags &= !FLAG_NONBLOCKING;
        }
        self
    }

    /// Set the maximum buffer capacity of the pipe in bytes.
    pub fn capacity(&mut self, capacity: usize) -> &mut Self {
        self.capacity = capacity;
        self
    }

    /// Create a new pipe using the current settings and return a reader and writer pair.
    pub fn build(&self) -> (PipeReader, PipeWriter) {
        let buffers = atomic::bounded(self.capacity);
        let reader_waker = Arc::new(Waker::new());
        let writer_waker = reader_waker.clone();

        (
            PipeReader {
                flags: self.flags,
                buffer: buffers.0,
                waker: reader_waker,
            },
            PipeWriter {
                flags: self.flags,
                buffer: buffers.1,
                waker: writer_waker,
            },
        )
    }
}

/// The reading end of a pipe.
pub struct PipeReader {
    flags: u8,
    buffer: atomic::Reader<u8>,
    waker: Arc<Waker>,
}

impl PipeReader {
    /// Set the non-blocking mode for this end of the pipe.
    ///
    /// If non-blocking mode is enabled, attempting to read from an empty pipe will return an `WouldBlock` error instead
    /// of blocking the current thread until data becomes available.
    pub fn set_nonblocking(&mut self, nonblocking: bool) {
        if nonblocking {
            self.flags |= FLAG_NONBLOCKING;
        } else {
            self.flags &= !FLAG_NONBLOCKING;
        }
    }
}

impl io::Read for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        loop {
            let len = self.buffer.pull(buf);

            // Successful read.
            if len > 0 {
                self.waker.notify_one();
                return Ok(len);
            }

            // Pipe is closed.
            if Arc::strong_count(&self.waker) == 1 {
                return Ok(0);
            }

            // Pipe is empty but we don't want to block.
            if self.flags & FLAG_NONBLOCKING != 0 {
                return Err(io::ErrorKind::WouldBlock.into());
            }

            // Pipe is empty and we do want to block.
            self.waker.wait();
        }
    }
}

/// The writing end of a pipe.
pub struct PipeWriter {
    flags: u8,
    buffer: atomic::Writer<u8>,
    waker: Arc<Waker>,
}

impl PipeWriter {
    /// Check if the reading end of the pipe has been closed.
    pub fn is_closed(&self) -> bool {
        Arc::strong_count(&self.waker) == 1
    }

    /// Set the non-blocking mode for this end of the pipe.
    ///
    /// If non-blocking mode is enabled, attempting to read from an empty pipe will return an `WouldBlock` error instead
    /// of blocking the current thread until data becomes available.
    pub fn set_nonblocking(&mut self, nonblocking: bool) {
        if nonblocking {
            self.flags |= FLAG_NONBLOCKING;
        } else {
            self.flags &= !FLAG_NONBLOCKING;
        }
    }
}

impl io::Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        loop {
            // Pipe is closed.
            if self.is_closed() {
                return Err(io::ErrorKind::BrokenPipe.into());
            }

            let len = self.buffer.push(buf);

            // Successful write.
            if len > 0 {
                self.waker.notify_one();
                return Ok(len);
            }

            // Pipe is full but we don't want to block.
            if self.flags & FLAG_NONBLOCKING != 0 {
                return Err(io::ErrorKind::WouldBlock.into());
            }

            // Pipe is full and we do want to block.
            self.waker.wait();
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Waker {
    mutex: Mutex<()>,
    condvar: Condvar,
}

impl Waker {
    fn new() -> Self {
        Self {
            mutex: Mutex::new(()),
            condvar: Condvar::new(),
        }
    }

    fn wait(&self) {
        let mut _lock = self.mutex.lock().unwrap();
        _lock = self.condvar.wait(_lock).unwrap();
    }

    fn notify_one(&self) {
        self.condvar.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Read, Write};
    use std::thread;
    use std::time::Duration;
    use super::*;

    #[test]
    fn read_write() {
        let (mut reader, mut writer) = pipe();

        assert_eq!(writer.write(b"hello world").unwrap(), 11);

        let mut buf = [0; 11];
        assert_eq!(reader.read(&mut buf).unwrap(), 11);
        assert_eq!(&buf, b"hello world");
    }

    #[test]
    fn read_empty_blocking() {
        let (mut reader, mut writer) = PipeBuilder::default()
            .capacity(16)
            .build();

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            let buf = [1; 1];
            writer.write(&buf).unwrap();
        });

        let mut buf = [0; 1];
        assert_eq!(reader.read(&mut buf).unwrap(), 1);
        assert_eq!(buf[0], 1);
    }

    #[test]
    fn read_nonblocking() {
        let (mut reader, _writer) = pipe();

        let mut buf = [0; 4];
        reader.set_nonblocking(true);
        assert_eq!(reader.read(&mut buf).err().unwrap().kind(), io::ErrorKind::WouldBlock);
    }

    #[test]
    fn write_nonblocking() {
        let (_reader, mut writer) = PipeBuilder::default()
            .capacity(16)
            .build();

        let buf = [0; 16];
        assert_eq!(writer.write(&buf).unwrap(), buf.len());

        writer.set_nonblocking(true);
        assert_eq!(writer.write(&buf).err().unwrap().kind(), io::ErrorKind::WouldBlock);
    }

    #[test]
    fn read_from_closed_pipe_returns_zero() {
        let (mut reader, _) = pipe();

        let mut buf = [0; 16];
        assert_eq!(reader.read(&mut buf).unwrap(), 0);
    }

    #[test]
    fn write_to_closed_pipe_returns_broken_pipe() {
        let (reader, mut writer) = pipe();

        assert!(!writer.is_closed());
        drop(reader);
        assert!(writer.is_closed());
        assert_eq!(writer.write(b"hi").err().unwrap().kind(), io::ErrorKind::BrokenPipe);
    }
}
