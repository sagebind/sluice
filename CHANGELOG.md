# Changelog

## 0.2.1 - 2018-08-02
- Fixed a logical error where pushing small amounts of data repeatedly into the buffer would overwrite existing data.

## 0.2.0 - 2018-05-17
0.2 is here! This release is a breaking change with API refactorings and improvements.

- Refactored module hierarchy to make more sense and to leave room for more data structures in the future.
- Add new traits for buffers that all buffer types can implement.
- Add new bounded atomic buffer implementation.

## 0.1.1 - 2018-02-24
- Fixed a critical logic error causing items to not be removable when the buffer length reached exactly the current capacity of the buffer.

## 0.1.0 - 2018-02-24
- Initial release.
