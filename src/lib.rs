//! Asynchronous byte buffers and pipes for concurrent I/O programming.
//!
//! ## Pipes
//!
//! The primary feature offered by Sluice are _pipes_, which are asynchronous
//! in-memory byte buffers that allow separate tasks to read and write from the
//! buffer in parallel.
//!
//! See the `pipe` module for details.

#![cfg_attr(test, feature(async_await))]

pub mod pipe;
