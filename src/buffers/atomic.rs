//! Atomic buffers useful for producer-consumer problems.
use arrays;
use arrays::CircularArray;
use buffers::{Buffer, ReadableBuffer, WritableBuffer};
use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::*;

/// Create a new atomic buffer with a given fixed capacity.
pub fn bounded<T>(capacity: usize) -> (Reader<T>, Writer<T>) {
    let inner = Arc::new(Inner::new(capacity));

    (
        Reader {
            inner: inner.clone(),
        },
        Writer {
            inner: inner,
        },
    )
}

/// Reading half of an atomic buffer.
pub struct Reader<T> {
    inner: Arc<Inner<T>>,
}

impl<T> Buffer<T> for Reader<T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    fn clear(&mut self) {
        let tail = self.inner.tail.load(Ordering::SeqCst);
        self.inner.head.store(tail, Ordering::SeqCst);
    }
}

impl<T: Copy> ReadableBuffer<T> for Reader<T> {
    fn copy_to(&self, dest: &mut [T]) -> usize {
        let head = self.inner.head.load(Ordering::SeqCst);
        let tail = self.inner.tail.load(Ordering::SeqCst);

        if head == tail {
            return 0;
        }

        unsafe {
            let array = &*self.inner.array.get();

            let slices = array.as_slices(head..tail);
            arrays::copy_from_seq(&slices, dest)
        }
    }

    fn consume(&mut self, count: usize) -> usize {
        // We can only consume as many elements as are in the buffer.
        let count = count.min(self.len());
        self.inner.head.fetch_add(count, Ordering::SeqCst);

        count
    }
}

/// Writing half of an atomic buffer.
pub struct Writer<T> {
    inner: Arc<Inner<T>>,
}

impl<T> Buffer<T> for Writer<T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    fn clear(&mut self) {
        let tail = self.inner.tail.load(Ordering::SeqCst);
        self.inner.head.store(tail, Ordering::SeqCst);
    }
}

impl<T: Copy> WritableBuffer<T> for Writer<T> {
    fn push(&mut self, src: &[T]) -> usize {
        let head = self.inner.head.load(Ordering::SeqCst);
        let tail = self.inner.tail.load(Ordering::SeqCst);

        if tail.wrapping_sub(head) == self.capacity() {
            return 0;
        }

        unsafe {
            let array = &mut *self.inner.array.get();

            let mut slices = array.as_slices_mut(tail..head);
            let pushed = arrays::copy_to_seq(src, &mut slices);

            self.inner.tail.fetch_add(pushed, Ordering::SeqCst);
            pushed
        }
    }
}

/// Contains the shared data between the reader and writer.
struct Inner<T> {
    array: UnsafeCell<CircularArray<T>>,
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl<T> Inner<T> {
    fn new(capacity: usize) -> Self {
        Self {
            array: unsafe {
                UnsafeCell::new(CircularArray::uninitialized(capacity))
            },
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }
}

impl<T> Buffer<T> for Inner<T> {
    fn len(&self) -> usize {
        let head = self.head.load(Ordering::SeqCst);
        let tail = self.tail.load(Ordering::SeqCst);

        // Even if `tail` overflows and becomes less than `head`, subtracting will underflow and result in the
        // correct length.
        tail.wrapping_sub(head)
    }

    #[inline]
    fn capacity(&self) -> usize {
        unsafe {
            (&*self.array.get()).len()
        }
    }

    fn clear(&mut self) {
        let tail = self.tail.load(Ordering::SeqCst);
        self.head.store(tail, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capacity() {
        let buffer = bounded::<u8>(16);

        assert_eq!(buffer.0.capacity(), 16);
        assert_eq!(buffer.1.capacity(), 16);
    }

    #[test]
    fn test_push() {
        let mut buffer = bounded::<u8>(16);

        assert!(buffer.0.is_empty());
        assert!(buffer.1.is_empty());

        let bytes = b"hello world";
        assert_eq!(buffer.1.push(bytes), bytes.len());

        assert!(!buffer.0.is_empty());
        assert!(!buffer.1.is_empty());

        assert_eq!(buffer.0.len(), bytes.len());
        assert_eq!(buffer.1.len(), bytes.len());
    }

    #[test]
    fn test_push_more_than_buffer() {
        let mut buffer = bounded::<u8>(2);
        assert_eq!(buffer.0.capacity(), 2);

        assert_eq!(buffer.1.push(&[100]), 1);
        assert_eq!(buffer.1.push(&[200]), 1);
        assert_eq!(buffer.1.push(&[210]), 0);
        assert_eq!(buffer.1.push(&[220]), 0);

        assert_eq!(buffer.0.len(), 2);
    }

    #[test]
    fn test_push_and_consume() {
        let mut buffer = bounded::<u8>(12);

        assert_eq!(buffer.1.push(b"hello world"), 11);

        assert_eq!(buffer.0.consume(6), 6);
        assert_eq!(buffer.0.len(), 5);

        assert_eq!(buffer.1.push(b" hello"), 6);

        assert_eq!(buffer.0.len(), 11);

        let mut dest = [0; 11];
        assert_eq!(buffer.0.copy_to(&mut dest), 11);
        assert_eq!(&dest, b"world hello");
    }

    #[test]
    fn test_pull_more_than_buffer() {
        let mut buffer = bounded(32);
        let bytes = b"hello world";
        buffer.1.push(bytes);

        let mut dst = [0; 1024];
        assert_eq!(buffer.0.pull(&mut dst), bytes.len());
        assert_eq!(&dst[0..bytes.len()], bytes);
        assert!(buffer.0.is_empty());
    }

    #[test]
    fn test_pull_less_than_buffer() {
        let mut buffer = bounded(32);
        let bytes = b"hello world";
        buffer.1.push(bytes);

        let mut dst = [0; 4];
        assert_eq!(buffer.0.pull(&mut dst), dst.len());
        assert_eq!(&dst, &bytes[0..4]);
        assert!(!buffer.0.is_empty());
        assert_eq!(buffer.0.len(), bytes.len() - dst.len());
    }
}
