//! Efficient ring buffer implementations for networking, audio, or queues.
mod arrays;
pub mod buffer;

/// A growable byte buffer implemented as a ring buffer.
pub type ByteBuffer = buffer::Buffer<u8>;
