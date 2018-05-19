//! Provides functions for dynamic array manipulation.
use std::ops::{Index, IndexMut, Range};

/// Allocate an uninitialized array of a given size.
///
/// Note that the contents of the array are not initialized and the values are undefined.
pub unsafe fn allocate<T>(len: usize) -> Box<[T]> {
    let mut vec = Vec::with_capacity(len);
    vec.set_len(len);
    vec.into_boxed_slice()
}

/// Copy as many elements as possible from one slice to another.
///
/// Returns the number of elements copied.
#[inline]
pub fn copy<T: Copy>(src: &[T], dest: &mut [T]) -> usize {
    let len = src.len().min(dest.len());
    (&mut dest[..len]).copy_from_slice(&src[..len]);
    len
}

/// Copy as many elements as possible from a slice of slices to a slice.
///
/// Returns the number of elements copied.
pub fn copy_from_seq<T: Copy>(src_seq: &[&[T]], dest: &mut [T]) -> usize {
    let mut copied = 0;

    for src in src_seq {
        if copied < dest.len() {
            copied += copy(src, &mut dest[copied..]);
        } else {
            break;
        }
    }

    copied
}

/// Copy as many elements as possible from a slice to a slice of slices.
///
/// Returns the number of elements copied.
pub fn copy_to_seq<T: Copy>(src: &[T], dest_seq: &mut [&mut [T]]) -> usize {
    let mut copied = 0;

    for dest in dest_seq {
        if copied < src.len() {
            copied += copy(&src[copied..], *dest);
        } else {
            break;
        }
    }

    copied
}

/// A heap-allocated circular array, useful for implementing ring buffers.
///
/// This array type uses a _virtual indexing_ system. Indexing into the array applies a virtual mapping such that any
/// index is always mapped to a valid position in the array. More than one virtual index may map to the same position.
pub struct CircularArray<T> {
    array: Box<[T]>,
    mask: usize,
}

impl<T> CircularArray<T> {
    /// Create a new array of a given length containing uninitialized data.
    pub unsafe fn uninitialized(len: usize) -> Self {
        let len = len.next_power_of_two();

        Self {
            array: allocate(len),
            mask: len - 1,
        }
    }

    /// Get the length of the array.
    #[inline]
    pub fn len(&self) -> usize {
        self.array.len()
    }

    /// Gets a pair of slices in the given range, wrapping around length.
    pub fn as_slices(&self, range: Range<usize>) -> [&[T]; 2] {
        let start = self.internal_index(range.start);
        let end = self.internal_index(range.end);

        if start < end {
            [&self.array[start..end], &[]]
        } else {
            [&self.array[start..], &self.array[..end]]
        }
    }

    /// Gets a pair of mutable slices in the given range, wrapping around length.
    pub fn as_slices_mut(&mut self, range: Range<usize>) -> [&mut [T]; 2] {
        let start = self.internal_index(range.start);
        let end = self.internal_index(range.end);

        if start < end {
            [&mut self.array[start..end], &mut []]
        } else {
            let (mid, right) = self.array.split_at_mut(start);
            let left = mid.split_at_mut(end).0;
            [right, left]
        }
    }

    /// Get the internal index the given virtual index is mapped to.
    #[inline]
    fn internal_index(&self, virtual_index: usize) -> usize {
        virtual_index & self.mask
    }
}

impl<T> AsRef<[T]> for CircularArray<T> {
    fn as_ref(&self) -> &[T] {
        &self.array
    }
}

impl<T> AsMut<[T]> for CircularArray<T> {
    fn as_mut(&mut self) -> &mut [T] {
        &mut self.array
    }
}

impl<T> Index<usize> for CircularArray<T> {
    type Output = T;

    fn index(&self, index: usize) -> &T {
        self.array.index(self.internal_index(index))
    }
}

impl<T> IndexMut<usize> for CircularArray<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        let index = self.internal_index(index);
        self.array.index_mut(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_from_seq_with_less_elements() {
        let chunks: [&[i32]; 3] = [&[], &[1, 2], &[3]];
        let mut dest = [0; 6];

        assert_eq!(copy_from_seq(&chunks, &mut dest), 3);
        assert_eq!(&dest, &[1, 2, 3, 0, 0, 0]);
    }

    #[test]
    fn copy_from_seq_with_more_elements() {
        let chunks: [&[i32]; 5] = [&[], &[1, 2], &[], &[3], &[4, 5, 6]];
        let mut dest = [0; 4];

        assert_eq!(copy_from_seq(&chunks, &mut dest), 4);
        assert_eq!(&dest, &[1, 2, 3, 4]);
    }

    #[test]
    fn copy_to_seq_with_less_elements() {
        let src = [1, 2, 3];
        let mut dest_1 = [0; 1];
        let mut dest_2 = [0; 4];

        {
            let mut dest: [&mut [u8]; 2] = [&mut dest_1, &mut dest_2];
            assert_eq!(copy_to_seq(&src, &mut dest), 3);
        }

        assert_eq!(&dest_1, &[1]);
        assert_eq!(&dest_2, &[2, 3, 0, 0]);
    }

    #[test]
    fn copy_to_seq_with_more_elements() {
        let src = [1, 2, 3, 4];
        let mut dest_1 = [0; 1];
        let mut dest_2 = [0; 2];

        {
            let mut dest: [&mut [u8]; 2] = [&mut dest_1, &mut dest_2];
            assert_eq!(copy_to_seq(&src, &mut dest), 3);
        }

        assert_eq!(&dest_1, &[1]);
        assert_eq!(&dest_2, &[2, 3]);
    }
}
