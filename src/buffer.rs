use arrays::{self, WrappingSlice};
use std::io::{self, Read, Write};

/// Growable ring buffer.
///
/// Optimized for repeated appending of bytes to the end and removing bytes from the front of the buffer.
#[derive(Clone)]
pub struct Buffer<T> {
    /// Backing array where elements are stored. Size is always a power of two.
    array: Box<[T]>,

    /// The "head" index into the backing array that marks the start of the buffer elements.
    ///
    /// This index may exceed the length of the backing array during the lifetime of the buffer, and is only ever
    /// incremented.
    head: usize,

    /// The "tail" index into the backing array that marks the end of the buffer elements.
    ///
    /// Same as `head`, this is incremented unbounded.
    tail: usize,
}

impl<T: Copy> Default for Buffer<T> {
    fn default() -> Buffer<T> {
        Buffer::new()
    }
}

impl<T: Copy> Buffer<T> {
    pub const DEFAULT_CAPACITY: usize = 4096;

    /// Create a new buffer with the default capacity.
    pub fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    /// Create a new buffer with a given minimum capacity pre-allocated.
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.next_power_of_two();
        Self {
            array: unsafe { arrays::allocate(capacity) },
            head: 0,
            tail: 0,
        }
    }

    /// Returns `true` if the buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        // The head and tail can only be equal to each other if: (a) the number of inserted elements over time is equal
        // to the number of removed elements over time, and is thus empty; or (b) exactly `usize::max_value()` elements
        // were inserted without being removed such that `tail` overflowed and wrapped around to equal `head`. This is
        // improbable since the buffer would have to be at least the size of max pointer value. If the OS does let you
        // allocate more memory than fits in a pointer, you have bigger problems.
        self.head == self.tail
    }

    /// Returns the number of elements in the buffer.
    #[inline]
    pub fn len(&self) -> usize {
        // Even if `tail` overflows and becomes less than `head`, subtracting will underflow and result in the correct
        // length.
        self.tail.wrapping_sub(self.head)
    }

    /// Returns the current capacity of the buffer.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.array.len()
    }

    /// Copy the given elements and insert them into the back of the buffer.
    ///
    /// Returns the number of elements pushed.
    pub fn push(&mut self, src: &[T]) -> usize {
        // If the number of bytes to add would exceed the capacity, grow the internal array first.
        let new_len = self.len() + src.len();
        if new_len > self.capacity() {
            self.resize(new_len);
        }

        let head_index = self.mask(self.head);
        let tail_index = self.mask(self.tail);

        let slices = self.array.wrapping_range_mut(tail_index, head_index);

        let mut pushed = arrays::copy(src, slices.0);
        pushed += arrays::copy(&src[pushed..], slices.1);

        self.tail = self.tail.wrapping_add(pushed);
        pushed
    }

    /// Pull bytes from the front of the buffer into the given location, up to the length of the destination buffer.
    ///
    /// Returns the number of elements pulled.
    pub fn pull(&mut self, dest: &mut [T]) -> usize {
        let count = self.copy_to(dest);
        self.consume(count)
    }

    /// Copy elements from the front of the buffer into the given slice.
    ///
    /// Returns the number of elements copied. If there are less elements in the buffer than the length of `dest`, then
    /// only part of `dest` will be written to.
    pub fn copy_to(&self, dest: &mut [T]) -> usize {
        if self.is_empty() {
            return 0;
        }

        let slices = self.array
            .wrapping_range(self.mask(self.head), self.mask(self.tail));

        let mut copied = arrays::copy(slices.0, dest);
        copied += arrays::copy(slices.1, &mut dest[copied..]);

        copied
    }

    /// Consume up to `count` elements from the front of the buffer and discards them.
    ///
    /// Returns the number of elements consumed, which may be less than `count` if `count` was greater than the number
    /// of elements in the buffer.
    ///
    /// This operation has a runtime cost of `O(1)`.
    pub fn consume(&mut self, count: usize) -> usize {
        // We can only consume as many elements as are in the buffer.
        let count = count.min(self.len());
        self.head = self.head.wrapping_add(count);
        count
    }

    /// Remove all elements from the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.tail = 0;
    }

    fn resize(&mut self, size: usize) {
        // Size must always be a power of 2.
        let size = size.next_power_of_two();

        let mut array = unsafe { arrays::allocate(size) };

        self.tail = self.copy_to(&mut array);
        self.head = 0;
        self.array = array;
    }

    #[inline]
    fn mask(&self, index: usize) -> usize {
        index & (self.capacity() - 1)
    }
}

impl Read for Buffer<u8> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        Ok(self.pull(buf))
    }
}

impl Write for Buffer<u8> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(self.push(buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Buffer;

    #[test]
    fn test_capacity() {
        let buffer = Buffer::<u8>::with_capacity(16);
        assert!(buffer.capacity() == 16);
    }

    #[test]
    fn test_push() {
        let mut buffer = Buffer::new();

        assert!(buffer.is_empty());

        let bytes = b"hello world";
        buffer.push(bytes);

        assert!(!buffer.is_empty());
        assert!(buffer.len() == bytes.len());
    }

    #[test]
    fn test_push_and_consume() {
        let mut buffer = Buffer::with_capacity(12);

        buffer.push(b"hello world");

        assert!(buffer.consume(6) == 6);
        assert!(buffer.len() == 5);

        buffer.push(b" hello");

        assert!(buffer.len() == 11);
    }

    #[test]
    fn test_pull_more_than_buffer() {
        let mut buffer = Buffer::new();
        let bytes = b"hello world";
        buffer.push(bytes);

        let mut dst = [0; 1024];
        assert!(buffer.pull(&mut dst) == bytes.len());
        assert!(&dst[0..bytes.len()] == bytes);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_pull_less_than_buffer() {
        let mut buffer = Buffer::new();
        let bytes = b"hello world";
        buffer.push(bytes);

        let mut dst = [0; 4];
        assert!(buffer.pull(&mut dst) == dst.len());
        assert!(&dst == &bytes[0..4]);
        assert!(!buffer.is_empty());
        assert!(buffer.len() == bytes.len() - dst.len());
    }

    #[test]
    fn test_force_resize() {
        let mut buffer = Buffer::with_capacity(8);

        buffer.push(b"hello");
        assert!(buffer.capacity() == 8);

        buffer.push(b" world");
        assert!(buffer.capacity() > 8);

        let mut out = [0; 11];
        buffer.copy_to(&mut out);
        assert!(&out == b"hello world");
    }
}
