//! Provides functions for dynamic array manipulation.

/// Allocate an uninitialized array of a given size.
///
/// Note that the contents of the array are not initialized and the values are undefined.
pub unsafe fn allocate<T>(len: usize) -> Box<[T]> {
    let mut vec = Vec::with_capacity(len);
    vec.set_len(len);
    vec.into_boxed_slice()
}

/// Copy as many elements as possible from one array to another.
///
/// Returns the number of elements copied.
#[inline]
pub fn copy<T: Copy>(src: &[T], dest: &mut [T]) -> usize {
    let len = src.len().min(dest.len());
    (&mut dest[..len]).copy_from_slice(&src[..len]);
    len
}

/// Extension trait for slices for working with wrapping ranges and indicies.
pub trait WrappingSlice<T> {
    /// Gets a pair of slices in the given range, wrapping around length.
    fn wrapping_range(&self, from: usize, to: usize) -> (&[T], &[T]);

    /// Gets a pair of mutable slices in the given range, wrapping around length.
    fn wrapping_range_mut(&mut self, from: usize, to: usize) -> (&mut [T], &mut [T]);
}

impl<T> WrappingSlice<T> for [T] {
    fn wrapping_range(&self, from: usize, to: usize) -> (&[T], &[T]) {
        if from < to {
            (&self[from..to], &[])
        } else {
            (&self[from..], &self[..to])
        }
    }

    fn wrapping_range_mut(&mut self, from: usize, to: usize) -> (&mut [T], &mut [T]) {
        if from < to {
            (&mut self[from..to], &mut [])
        } else {
            let (mid, right) = self.split_at_mut(from);
            let left = mid.split_at_mut(to).0;
            (right, left)
        }
    }
}
