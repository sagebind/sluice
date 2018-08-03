use arrays::{self, CircularArray};
use buffers::{Buffer, ReadableBuffer, WritableBuffer};

/// Growable ring buffer.
///
/// Optimized for repeated appending of bytes to the end and removing bytes from the front of the buffer.
pub struct UnboundedBuffer<T> {
    /// Backing array where elements are stored. Size is always a power of two.
    array: CircularArray<T>,

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

impl<T: Copy> Default for UnboundedBuffer<T> {
    fn default() -> UnboundedBuffer<T> {
        UnboundedBuffer::new()
    }
}

impl<T: Copy> UnboundedBuffer<T> {
    pub const DEFAULT_CAPACITY: usize = 4096;

    /// Create a new unbounded buffer with the default capacity.
    pub fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    /// Create a new unbounded buffer with a given minimum capacity pre-allocated.
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.next_power_of_two();
        Self {
            array: unsafe { CircularArray::uninitialized(capacity) },
            head: 0,
            tail: 0,
        }
    }

    fn resize(&mut self, size: usize) {
        let mut array = unsafe { CircularArray::uninitialized(size) };

        let copied = self.copy_to(array.as_mut());
        debug_assert_eq!(copied, self.len());

        self.tail = copied;
        self.head = 0;
        self.array = array;
    }
}

impl<T> Buffer<T> for UnboundedBuffer<T> {
    #[inline]
    fn is_empty(&self) -> bool {
        // The head and tail can only be equal to each other if: (a) the number of inserted elements over time is equal
        // to the number of removed elements over time, and is thus empty; or (b) exactly `usize::max_value()` elements
        // were inserted without being removed such that `tail` overflowed and wrapped around to equal `head`. This is
        // improbable since the buffer would have to be at least the size of max pointer value. If the OS does let you
        // allocate more memory than fits in a pointer, you have bigger problems.
        self.head == self.tail
    }

    #[inline]
    fn len(&self) -> usize {
        // Even if `tail` overflows and becomes less than `head`, subtracting will underflow and result in the correct
        // length.
        self.tail.wrapping_sub(self.head)
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.array.len()
    }

    fn clear(&mut self) {
        self.head = 0;
        self.tail = 0;
    }
}

impl<T: Copy> ReadableBuffer<T> for UnboundedBuffer<T> {
    fn copy_to(&self, dest: &mut [T]) -> usize {
        if self.head == self.tail {
            return 0;
        }

        let slices = self.array.as_slices(self.head..self.tail);
        arrays::copy_from_seq(&slices, dest)
    }

    fn consume(&mut self, count: usize) -> usize {
        // We can only consume as many elements as are in the buffer.
        let count = count.min(self.len());
        self.head = self.head.wrapping_add(count);
        count
    }
}

impl<T: Copy> WritableBuffer<T> for UnboundedBuffer<T> {
    fn push(&mut self, src: &[T]) -> usize {
        // If the number of bytes to add would exceed the capacity, grow the internal array first.
        let new_len = self.len() + src.len();
        if new_len > self.capacity() {
            self.resize(new_len);
        }

        let mut slices = self.array.as_slices_mut(self.tail..self.head);
        let pushed = arrays::copy_to_seq(src, &mut slices);

        self.tail = self.tail.wrapping_add(pushed);
        debug_assert_eq!(pushed, src.len());
        pushed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capacity() {
        let buffer = UnboundedBuffer::<u8>::with_capacity(16);
        assert_eq!(buffer.capacity(), 16);
    }

    #[test]
    fn test_push() {
        let mut buffer = UnboundedBuffer::new();

        assert!(buffer.is_empty());

        let bytes = b"hello world";
        assert_eq!(buffer.push(bytes), bytes.len());

        assert!(!buffer.is_empty());
        assert_eq!(buffer.len(), bytes.len());
    }

    #[test]
    fn test_push_and_consume() {
        let mut buffer = UnboundedBuffer::with_capacity(12);

        assert_eq!(buffer.push(b"hello world"), 11);

        assert_eq!(buffer.consume(6), 6);
        assert_eq!(buffer.len(), 5);

        assert_eq!(buffer.push(b" hello"), 6);

        assert_eq!(buffer.len(), 11);
    }

    #[test]
    fn test_push_a_lot() {
        let mut buffer = UnboundedBuffer::new();
        let bytes = "heavyweight;".repeat(1000).into_bytes();

        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.push(&bytes), bytes.len());
        assert_eq!(buffer.len(), bytes.len());
        assert_eq!(buffer.consume(bytes.len()), bytes.len());
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_pull_more_than_buffer() {
        let mut buffer = UnboundedBuffer::new();
        let bytes = b"hello world";
        assert_eq!(buffer.push(bytes), 11);

        let mut dst = [0; 1024];
        assert_eq!(buffer.pull(&mut dst), bytes.len());
        assert_eq!(&dst[0..bytes.len()], bytes);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_pull_less_than_buffer() {
        let mut buffer = UnboundedBuffer::new();
        let bytes = b"hello world";
        buffer.push(bytes);

        let mut dst = [0; 4];
        assert_eq!(buffer.pull(&mut dst), dst.len());
        assert_eq!(&dst, &bytes[0..4]);
        assert!(!buffer.is_empty());
        assert_eq!(buffer.len(), bytes.len() - dst.len());
    }

    #[test]
    fn test_force_resize() {
        let mut buffer = UnboundedBuffer::with_capacity(8);

        buffer.push(b"hello");
        assert_eq!(buffer.capacity(), 8);

        buffer.push(b" world");
        assert!(buffer.capacity() > 8);

        let mut out = [0; 11];
        buffer.copy_to(&mut out);
        assert_eq!(&out, b"hello world");
    }
}
