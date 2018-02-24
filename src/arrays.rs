//! Provides functions for dynamic array manipulation.

/// Allocate an uninitialized array of a given size.
///
/// Note that the contents of the array are not initialized and the values are undefined.
pub unsafe fn allocate<T>(len: usize) -> Box<[T]> {
    let mut vec = Vec::with_capacity(len);
    vec.set_len(len);
    vec.into_boxed_slice()
}

/// Copy elements from one array to another in a range.
///
/// Panics if there are less than `len` items in either of the given regions.
#[inline]
pub fn copy<T: Copy>(src: &[T], src_offset: usize, dest: &mut [T], dest_offset: usize, len: usize) {
    (&mut dest[dest_offset .. dest_offset + len]).copy_from_slice(&src[src_offset .. src_offset + len])
}
