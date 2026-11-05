//! Cancellation token shared with the UI.

use std::sync::atomic::{AtomicBool, Ordering};

/// Cooperative cancellation flag.
///
/// Workers test it once per directory, so cancelling a scan costs at most one
/// in-flight `read_dir` call. The flag is reset explicitly before a new scan
/// rather than on drop, so a late-arriving cancel cannot leak into the next run.
#[derive(Debug, Default)]
pub struct CancelToken {
    cancelled: AtomicBool,
}

impl CancelToken {
    /// Creates a non-cancelled token.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation. Idempotent.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Whether a cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Clears the flag for the next scan.
    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_is_sticky_until_reset() {
        let token = CancelToken::new();
        assert!(!token.is_cancelled());
        token.cancel();
        token.cancel();
        assert!(token.is_cancelled());
        token.reset();
        assert!(!token.is_cancelled());
    }
}
