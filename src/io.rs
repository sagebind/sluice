//! Provides types specifically designed for working with bytes and I/O.

use buffers::{atomic, ReadableBuffer, WritableBuffer};
use internal::sync::Signal;
use std::io;
use std::sync::atomic::*;
use std::sync::Arc;

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
        let shared = Arc::<PipeShared>::default();

        (
            PipeReader {
                flags: self.flags,
                buffer: buffers.0,
                shared: shared.clone(),
            },
            PipeWriter {
                flags: self.flags,
                buffer: buffers.1,
                shared: shared,
            },
        )
    }
}

/// The reading end of a pipe.
pub struct PipeReader {
    flags: u8,
    buffer: atomic::Reader<u8>,
    shared: Arc<PipeShared>,
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
                self.shared.full_signal.notify();
                return Ok(len);
            }

            // Pipe is empty, check if it is closed.
            if self.shared.drop_flag.load(Ordering::SeqCst) {
                return Ok(0);
            }

            // Pipe is empty, but we don't want to block.
            if self.flags & FLAG_NONBLOCKING != 0 {
                return Err(io::ErrorKind::WouldBlock.into());
            }

            // Pipe is empty, and we do want to block.
            self.shared.empty_signal.wait();
        }
    }
}

impl Drop for PipeReader {
    fn drop(&mut self) {
        self.shared.drop_flag.store(true, Ordering::SeqCst);
        self.shared.full_signal.notify();
    }
}

/// The writing end of a pipe.
pub struct PipeWriter {
    flags: u8,
    buffer: atomic::Writer<u8>,
    shared: Arc<PipeShared>,
}

impl PipeWriter {
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
            // Early check for closed pipe.
            if self.shared.drop_flag.load(Ordering::SeqCst) {
                return Err(io::ErrorKind::BrokenPipe.into());
            }

            let len = self.buffer.push(buf);

            // Successful write.
            if len > 0 {
                self.shared.empty_signal.notify();
                return Ok(len);
            }

            // Pipe is full, but we don't want to block.
            if self.flags & FLAG_NONBLOCKING != 0 {
                return Err(io::ErrorKind::WouldBlock.into());
            }

            // Pipe is full, and we do want to block.
            self.shared.full_signal.wait();
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for PipeWriter {
    fn drop(&mut self) {
        self.shared.drop_flag.store(true, Ordering::SeqCst);
        self.shared.empty_signal.notify();
    }
}

/// Used to coordinate synchronization between both ends of a pipe.
#[derive(Default)]
struct PipeShared {
    empty_signal: Signal,
    full_signal: Signal,
    drop_flag: AtomicBool,
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
        let (_, mut writer) = pipe();

        assert_eq!(writer.write(b"hi").err().unwrap().kind(), io::ErrorKind::BrokenPipe);
    }
}
