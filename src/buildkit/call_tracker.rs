use std::sync::atomic::{AtomicU64, Ordering};

/// Thread-safe call ID generator for gRPC service calls
pub struct CallTracker {
    counter: AtomicU64,
}

impl CallTracker {
    pub const fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }

    pub fn next_id(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::SeqCst)
    }
}

impl Default for CallTracker {
    fn default() -> Self {
        Self::new()
    }
}
