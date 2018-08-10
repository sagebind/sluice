//! Synchronization primitives.

use std::sync::atomic::*;
use std::sync::{Condvar, Mutex};

/// A synchronization primitive used to put threads to sleep until another thread wakes it up.
pub struct Signal {
    lock: Mutex<()>,
    condvar: Condvar,
    notified: AtomicBool,
    waiting: AtomicUsize,
}

impl Signal {
    pub fn new() -> Self {
        Self {
            lock: Mutex::new(()),
            condvar: Condvar::new(),
            notified: AtomicBool::default(),
            waiting: AtomicUsize::default(),
        }
    }

    pub fn notify(&self) {
        // Set the notify flag.
        self.notified.store(true, Ordering::SeqCst);

        // If any threads are waiting, wake one up.
        if self.waiting.load(Ordering::SeqCst) > 0 {
            // Acquire the mutex to coordinate waking up a thread.
            let _guard = self.lock.lock().unwrap();
            self.condvar.notify_one();
        }
    }

    pub fn wait(&self) {
        // Fast path.
        if self.notified.swap(false, Ordering::SeqCst) {
            return;
        }

        // Indicate we have begun waiting.
        self.waiting.fetch_add(1, Ordering::SeqCst);

        // Acquire the mutex to coordinate waiting.
        let mut guard = self.lock.lock().unwrap();

        // Ensure the notify flag was not just set, then wait loop to ignore spurious wake-ups.
        while !self.notified.swap(false, Ordering::SeqCst) {
            guard = self.condvar.wait(guard).unwrap();
        }

        // We're finished waiting.
        drop(guard);
        self.waiting.fetch_sub(1, Ordering::SeqCst);
    }
}
