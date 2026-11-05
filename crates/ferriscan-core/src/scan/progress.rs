//! Lock-free counters and the current-path cell read by the progress reporter.

use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

/// Lifecycle of a scan, mirrored in the UI.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ScanState {
    /// Nothing running.
    Idle = 0,
    /// Options validated, arenas allocated.
    Preparing = 1,
    /// Directory enumeration in progress.
    Scanning = 2,
    /// Aggregation of sizes and histograms.
    Finalizing = 3,
    /// A snapshot is available.
    Ready = 4,
    /// Cancellation requested, workers draining.
    Cancelling = 5,
    /// Stopped by the user; partial results are kept.
    Cancelled = 6,
    /// Ended without a snapshot.
    Failed = 7,
}

impl ScanState {
    /// Decodes the atomic representation, falling back to `Idle`.
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Preparing,
            2 => Self::Scanning,
            3 => Self::Finalizing,
            4 => Self::Ready,
            5 => Self::Cancelling,
            6 => Self::Cancelled,
            7 => Self::Failed,
            _ => Self::Idle,
        }
    }

    /// Whether the state allows starting a new scan.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Idle | Self::Ready | Self::Cancelled | Self::Failed
        )
    }
}

/// Shared progress counters.
///
/// Workers only touch relaxed atomics; the reporter thread reads them at 10 Hz.
/// The current path is behind a mutex but is written once per directory, which is
/// negligible compared to the enumeration syscalls.
#[derive(Debug, Default)]
pub struct Progress {
    state: AtomicU8,
    items: AtomicU64,
    bytes: AtomicU64,
    errors: AtomicU64,
    current: Mutex<Option<Box<str>>>,
}

/// A consistent-enough copy of the counters for one progress event.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProgressSnapshot {
    /// Lifecycle state.
    pub state: u8,
    /// Entries indexed so far.
    pub items: u64,
    /// Logical bytes seen so far.
    pub bytes: u64,
    /// Directories or entries that failed.
    pub errors: u64,
    /// Directory currently being read, if any.
    pub current_path: Option<String>,
}

impl Progress {
    /// Creates an idle progress cell.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Publishes a new state.
    pub fn set_state(&self, state: ScanState) {
        self.state.store(state as u8, Ordering::Release);
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> ScanState {
        ScanState::from_u8(self.state.load(Ordering::Acquire))
    }

    /// Adds the result of one directory.
    pub fn add(&self, items: u64, bytes: u64) {
        self.items.fetch_add(items, Ordering::Relaxed);
        self.bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Increments the error counter.
    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Sets the directory currently being read.
    pub fn set_current_path(&self, path: &std::path::Path) {
        let mut guard = self.current.lock().unwrap_or_else(PoisonError::into_inner);
        *guard = Some(path.display().to_string().into_boxed_str());
    }

    /// Copies the counters for one event.
    #[must_use]
    pub fn snapshot(&self) -> ProgressSnapshot {
        ProgressSnapshot {
            state: self.state.load(Ordering::Acquire),
            items: self.items.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            current_path: self
                .current
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_deref()
                .map(str::to_owned),
        }
    }

    /// Resets counters before a new scan.
    pub fn reset(&self) {
        self.state
            .store(ScanState::Preparing as u8, Ordering::Release);
        self.items.store(0, Ordering::Relaxed);
        self.bytes.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);
        *self.current.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_accumulate() {
        let progress = Progress::new();
        progress.add(3, 100);
        progress.add(2, 50);
        progress.record_error();
        let snap = progress.snapshot();
        assert_eq!(snap.items, 5);
        assert_eq!(snap.bytes, 150);
        assert_eq!(snap.errors, 1);
        assert_eq!(snap.state, ScanState::Idle as u8);
    }

    #[test]
    fn reset_clears_counters() {
        let progress = Progress::new();
        progress.add(1, 1);
        progress.reset();
        let snap = progress.snapshot();
        assert_eq!(snap.items, 0);
        assert_eq!(snap.state, ScanState::Preparing as u8);
    }
}
