//! Synchronization primitives.

use std::sync::atomic::*;
use std::sync::{Condvar, Mutex};

/// A synchronization primitive used to put threads to sleep until another thread wakes it up.
pub struct Signal {
    lock: Mutex<()>,
    condvar: Condvar,
    notified: AtomicBool,
}

impl Signal {
    pub fn new() -> Self {
        Self {
            lock: Mutex::new(()),
            condvar: Condvar::new(),
            notified: AtomicBool::default(),
        }
    }

    pub fn notify(&self) {
        // Set the notify flag.
        self.notified.store(true, Ordering::SeqCst);

        // Acquire the mutex to coordinate waking up a thread.
        let _guard = self.lock.lock().unwrap();
        self.condvar.notify_one();
    }

    pub fn wait(&self) {
        // Fast path.
        if self.notified.swap(false, Ordering::SeqCst) {
            return;
        }

        // Acquire the mutex to coordinate waiting.
        let mut guard = self.lock.lock().unwrap();

        // Ensure the notify flag was not just set, then wait loop to ignore spurious wake-ups.
        while !self.notified.swap(false, Ordering::SeqCst) {
            guard = self.condvar.wait(guard).unwrap();
        }
    }
}
