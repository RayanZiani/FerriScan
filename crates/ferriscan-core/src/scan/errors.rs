//! Error codes and the bounded error sink used during a scan.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use rustc_hash::FxHashMap;

/// Stable identifiers for every failure the engine can report.
///
/// The frontend maps these to localized messages; the raw OS text is only shown
/// inside an expandable "technical details" block.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum ErrorCode {
    /// Permission denied by the OS or an ACL.
    PermissionDenied,
    /// The path disappeared between enumeration and use.
    NotFound,
    /// The path exceeds a platform limit.
    PathTooLong,
    /// A symlink or reparse cycle was detected.
    Loop,
    /// Generic I/O failure.
    Io,
    /// Invalid input, including malformed paths.
    Invalid,
    /// The entry is locked by another process.
    Busy,
    /// The scan was cancelled.
    Cancelled,
    /// The operation is not supported on this platform or filesystem.
    Unsupported,
    /// The entry changed since it was planned (TOCTOU guard).
    Changed,
    /// Not enough free space at the destination.
    NoSpace,
}

impl ErrorCode {
    /// Stable wire representation used by DTOs and localization keys.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PermissionDenied => "E_PERM",
            Self::NotFound => "E_NOTFOUND",
            Self::PathTooLong => "E_TOOLONG",
            Self::Loop => "E_LOOP",
            Self::Io => "E_IO",
            Self::Invalid => "E_INVALID",
            Self::Busy => "E_BUSY",
            Self::Cancelled => "E_CANCELLED",
            Self::Unsupported => "E_UNSUPPORTED",
            Self::Changed => "E_CHANGED",
            Self::NoSpace => "E_NOSPC",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One recorded problem, with the path that caused it.
#[derive(Clone, Debug)]
pub struct RecordedError {
    /// Stable code.
    pub code: ErrorCode,
    /// Path as logged (relative or hashed, depending on the policy).
    pub path: Box<str>,
    /// Raw OS message.
    pub message: Box<str>,
}

/// Keeps a bounded sample of scan errors plus aggregate counters.
///
/// A scan over a locked volume can produce hundreds of thousands of `E_PERM`
/// errors; storing them all would blow the memory budget. The sink keeps the
/// first [`capacity`](Self::new) samples and counts every occurrence per code.
#[derive(Debug)]
pub struct ErrorSink {
    capacity: usize,
    total: AtomicU64,
    samples: Mutex<Vec<RecordedError>>,
    by_code: Mutex<FxHashMap<ErrorCode, u64>>,
}

impl ErrorSink {
    /// Creates a sink that retains at most `capacity` samples.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            total: AtomicU64::new(0),
            samples: Mutex::new(Vec::new()),
            by_code: Mutex::new(FxHashMap::default()),
        }
    }

    /// Records one error; the sample is dropped when the buffer is full.
    pub fn record(&self, code: ErrorCode, path: impl Into<Box<str>>, message: impl Into<Box<str>>) {
        self.total.fetch_add(1, Ordering::Relaxed);
        {
            let mut counts = recover(self.by_code.lock());
            *counts.entry(code).or_insert(0) += 1;
        }
        let mut samples = recover(self.samples.lock());
        if samples.len() < self.capacity {
            samples.push(RecordedError {
                code,
                path: path.into(),
                message: message.into(),
            });
        }
    }

    /// Total errors seen, including ones not retained as samples.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    /// Snapshot of the retained samples.
    #[must_use]
    pub fn samples(&self) -> Vec<RecordedError> {
        recover(self.samples.lock()).clone()
    }

    /// Per-code counters.
    #[must_use]
    pub fn counts_by_code(&self) -> FxHashMap<ErrorCode, u64> {
        recover(self.by_code.lock()).clone()
    }
}

/// Poisoning is intentionally ignored: a panic while recording an error is
/// already a bug, and losing one sample is better than killing the scan.
fn recover<T>(result: std::sync::LockResult<T>) -> T {
    result.unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable_strings() {
        assert_eq!(ErrorCode::PermissionDenied.as_str(), "E_PERM");
        assert_eq!(ErrorCode::NoSpace.to_string(), "E_NOSPC");
    }

    #[test]
    fn sink_bounds_samples_but_counts_all() {
        let sink = ErrorSink::new(2);
        for i in 0..10 {
            sink.record(ErrorCode::PermissionDenied, format!("/p{i}"), "denied");
        }
        assert_eq!(sink.total(), 10);
        assert_eq!(sink.samples().len(), 2);
        assert_eq!(sink.counts_by_code()[&ErrorCode::PermissionDenied], 10);
    }
}
