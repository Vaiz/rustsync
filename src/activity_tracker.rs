use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct ActivityTracker {
    processing_dirs: Arc<std::sync::atomic::AtomicI64>,
}

impl ActivityTracker {
    pub(crate) fn new(val: i64) -> Self {
        Self {
            processing_dirs: Arc::new(std::sync::atomic::AtomicI64::new(val))
        }
    }

    pub(crate) fn push(&self) {
        self.processing_dirs.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    pub(crate) fn pop(&self) -> bool {
        let prev = self.processing_dirs.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        prev == 1
    }
}