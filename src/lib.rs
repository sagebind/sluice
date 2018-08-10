//! Ringtail is a collection of buffers and queues, useful for networking, thread communication, and real-time
//! programming.
//!
//! Provided data structures are designed for efficiency first and foremost, so some common operations you might expect
//! of queues may be missing in order to allow certain optimizations. These are not general-purpose structures; several
//! versions of one structure may be provided with different trade-offs.
//!
//! ## Buffers
//!
//! In Ringtail, a _buffer_ is a queue optimized for reading and writing multiple elements in bulk, like an in-memory
//! version of an I/O stream.

pub mod buffers;
pub mod io;

mod internal;
