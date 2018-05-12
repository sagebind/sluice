//! Ring buffers for inserting and removing primitive types in bulk.
//!
//! Buffers are designed for reading and writing bytes in memory, and are useful as networking buffers, audio streams,
//! or as in-memory byte pipes between threads.
pub mod atomic;
pub mod unbounded;

/// Base trait that all buffers implement.
pub trait Buffer<T> {
    /// Returns `true` if the buffer is empty.
    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of elements in the buffer.
    fn len(&self) -> usize;

    /// Returns the current capacity of the buffer.
    fn capacity(&self) -> usize;

    /// Clears all elements from the buffer and resets the length to zero.
    fn clear(&mut self);
}

/// A buffer that can be read from.
pub trait ReadableBuffer<T>: Buffer<T> {
    /// Pull elements from the front of the buffer into the given location, up to the length of the destination buffer.
    ///
    /// Returns the number of elements pulled.
    fn pull(&mut self, dest: &mut [T]) -> usize where T: Copy {
        let count = self.copy_to(dest);
        self.consume(count)
    }

    /// Copy elements from the front of the buffer into the given slice.
    ///
    /// Returns the number of elements copied. If there are less elements in the buffer than the length of `dest`, then
    /// only part of `dest` will be written to.
    fn copy_to(&self, dest: &mut [T]) -> usize where T: Copy;

    /// Consume up to `count` elements from the front of the buffer and discards them.
    ///
    /// Returns the number of elements consumed, which may be less than `count` if `count` was greater than the number
    /// of elements in the buffer.
    fn consume(&mut self, count: usize) -> usize;
}

pub trait WritableBuffer<T>: Buffer<T> {
    /// Copy the given elements and insert them into the back of the buffer.
    ///
    /// Returns the number of elements pushed.
    fn push(&mut self, src: &[T]) -> usize;
}
