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
        self.notified.store(true, Ordering::SeqCst);
        self.condvar.notify_one();
    }

    pub fn wait(&self) {
        if self.notified.swap(false, Ordering::SeqCst) {
            return;
        }

        let mut guard = self.lock.lock().unwrap();
        loop {
            guard = self.condvar.wait(guard).unwrap();

            if self.notified.swap(false, Ordering::SeqCst) {
                return;
            }
        }
    }
}
