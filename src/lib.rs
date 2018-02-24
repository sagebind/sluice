use std::io::{self, Read, Write};

mod arrays;


/// Growable byte buffer implemented as a ring buffer.
///
/// Optimized for repeated appending of bytes to the end and removing bytes from the front of the buffer.
#[derive(Clone, Debug)]
pub struct Buffer {
    array: Box<[u8]>,
    head: usize,
    len: usize,
}

impl Default for Buffer {
    fn default() -> Buffer {
        Buffer::new()
    }
}

impl Buffer {
    pub const DEFAULT_CAPACITY: usize = 4096;

    /// Create a new buffer with the default capacity.
    pub fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    /// Create a new buffer with a given minimum capacity pre-allocated.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            array: unsafe {
                arrays::allocate(capacity.next_power_of_two())
            },
            head: 0,
            len: 0,
        }
    }

    /// Returns `true` if the buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the number of bytes in the buffer.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns the current capacity of the buffer in bytes.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.array.len()
    }

    /// Calculate the internal offset of the given byte position.
    #[inline]
    fn offset(&self, index: usize) -> usize {
        (index + self.head) & (self.capacity() - 1)
    }

    /// Copy bytes from the front of the buffer into the given slice.
    ///
    /// Returns the number of bytes copied. If there are less bytes in the buffer than the length of `dest`, then only
    /// part of `dest` will be written to.
    pub fn copy_to(&self, dest: &mut [u8]) -> usize {
        // Determine the number of bytes to copy.
        let count = dest.len().min(self.len);

        // Nothing to do.
        if count == 0 {
            return 0;
        }

        // Current buffer is wrapped; copy head segment and tail segment separately.
        let tail = self.offset(count);
        if tail <= self.head {
            let head_len = self.capacity() - self.head;
            arrays::copy(&self.array, self.head, dest, 0, head_len);
            arrays::copy(&self.array, 0, dest, head_len, tail);
        }

        // Buffer is contiguous; copy in one step.
        else {
            arrays::copy(&self.array, self.head, dest, 0, count);
        }

        count
    }

    /// Consume up to `count` bytes from the front of the buffer and discard them.
    ///
    /// Returns the number of bytes consumed, which may be less than `count` if `count` was greater than the number of
    /// bytes in the buffer.
    ///
    /// This operation has a runtime cost of `O(1)`.
    pub fn consume(&mut self, count: usize) -> usize {
        let count = count.min(self.len);

        self.head = self.offset(count);
        self.len -= count;

        count
    }

    /// Copy the given bytes and insert them into the back of the buffer.
    pub fn push(&mut self, src: &[u8]) {
        let new_len = self.len + src.len();

        // If the number of bytes to add would exceed the capacity, grow the internal array first.
        if new_len > self.capacity() {
            let new_capacity = new_len.next_power_of_two();
            let mut new_array = unsafe {
                arrays::allocate(new_capacity)
            };

            self.copy_to(&mut new_array);
            self.array = new_array;
            self.head = 0;
        }

        // Calculate how much of `src` should be copied to which regions.
        let head_available = self.capacity().checked_sub(self.head + self.len).unwrap_or(0);
        let copy_to_head = src.len().min(head_available);
        let copy_to_tail = src.len() - copy_to_head;

        if copy_to_head > 0 {
            let tail = self.offset(self.len);
            arrays::copy(src, 0, &mut self.array, tail, copy_to_head);
        }

        if copy_to_tail > 0 {
            arrays::copy(src, copy_to_head, &mut self.array, 0, copy_to_tail);
        }

        self.len = new_len;
    }

    /// Pull bytes from the front of the buffer into the given location, up to the length of the destination buffer.
    ///
    /// Returns the number of bytes pulled.
    pub fn pull(&mut self, dest: &mut [u8]) -> usize {
        let count = self.copy_to(dest);
        self.consume(count)
    }

    /// Remove all bytes from the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }
}

impl Read for Buffer {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        Ok(self.pull(buf))
    }
}

impl Write for Buffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.push(buf);
        Ok(buf.len())
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
        let buffer = Buffer::with_capacity(16);
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
