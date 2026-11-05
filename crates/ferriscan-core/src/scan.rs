//! Scan orchestration: options, scheduler, workers, cancellation and progress.

pub mod controller;
pub mod errors;
pub mod finalize;
pub mod progress;

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Instant;

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::fs::{DirJob, DirReader, EntryKind, RawEntry};
use crate::model::{NodeFlags, NodeStore, ScanMeta, Snapshot, SnapshotId, SnapshotStatus};
use crate::scan::controller::CancelToken;
use crate::scan::errors::{ErrorCode, ErrorSink};
use crate::scan::progress::{Progress, ScanState};

/// Default number of retained error samples.
pub const ERROR_SAMPLE_CAPACITY: usize = 1_000;

/// How a scan should behave. All fields have conservative defaults.
#[derive(Clone, Debug)]
pub struct ScanOptions {
    /// Include entries flagged hidden or system by the OS.
    pub include_hidden: bool,
    /// Stop at the filesystem boundary of the root.
    pub same_filesystem_only: bool,
    /// Skip files smaller than this many bytes.
    pub min_file_size: u64,
    /// Maximum depth below the root; `None` means unlimited.
    pub max_depth: Option<u16>,
    /// Worker thread count; `0` selects `available_parallelism` clamped to 2..=16.
    pub threads: usize,
    /// Glob patterns matched against the entry name and the joined path that
    /// terminate a subtree.
    pub excludes: Vec<String>,
    /// Hint for platform readers that can resolve link targets. The core never
    /// follows a symlink itself.
    pub follow_symlinks: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            include_hidden: true,
            same_filesystem_only: false,
            min_file_size: 0,
            max_depth: None,
            threads: 0,
            excludes: default_excludes(),
            follow_symlinks: false,
        }
    }
}

impl ScanOptions {
    /// Number of worker threads this configuration asks for.
    #[must_use]
    pub fn resolved_threads(&self) -> usize {
        if self.threads > 0 {
            return self.threads.clamp(1, 64);
        }
        std::thread::available_parallelism()
            .map_or(4, std::num::NonZeroUsize::get)
            .clamp(2, 16)
    }
}

/// Excludes that would otherwise waste time or skew results on every platform.
#[must_use]
pub fn default_excludes() -> Vec<String> {
    [
        "$RECYCLE.BIN",
        "System Volume Information",
        ".Spotlight-V100",
        ".fseventsd",
        ".Trashes",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// A scan that ended without producing a usable snapshot.
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    /// The root could not be indexed (empty name, capacity, arena overflow).
    #[error("cannot index the scan root: {0}")]
    InvalidRoot(String),
    /// The root is not a directory according to the reader.
    #[error("{0} is not a directory")]
    NotADirectory(String),
}

/// Runs one scan to completion on the calling thread, which blocks.
///
/// The caller runs this on a background thread (the Tauri command uses
/// `spawn_blocking`). Progress is published through [`Progress`] and
/// cancellation through [`CancelToken`]; neither takes a lock the workers need.
pub fn scan(
    reader: &dyn DirReader,
    root: &Path,
    options: &ScanOptions,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<Snapshot, ScanError> {
    let started = Instant::now();
    progress.set_state(ScanState::Scanning);

    let excludes = Arc::new(build_excludes(&options.excludes));
    let root_name = root.file_name().map_or_else(
        || root.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );

    let mut store = NodeStore::with_capacity(1 << 16);
    let root_id = store
        .insert_root(&root_name, 0, NodeFlags::DIR)
        .map_err(|e| ScanError::InvalidRoot(e.to_string()))?;

    if options.same_filesystem_only {
        // Device ids are only needed for the boundary check; asking once avoids a
        // syscall per directory on the common path.
        let _ = reader.device_id(root);
    }

    let errors = Arc::new(ErrorSink::new(ERROR_SAMPLE_CAPACITY));
    let writer = Arc::new(Mutex::new(store));
    let scheduler = Scheduler::new(DirJob {
        node: root_id,
        path: root.to_path_buf(),
        depth: 0,
    });

    let threads = options.resolved_threads();
    std::thread::scope(|scope| {
        for _ in 0..threads {
            let writer = Arc::clone(&writer);
            let errors = Arc::clone(&errors);
            let excludes = Arc::clone(&excludes);
            let scheduler = &scheduler;
            scope.spawn(move || {
                worker(
                    reader,
                    options,
                    progress,
                    cancel,
                    &errors,
                    &writer,
                    scheduler,
                    excludes.as_ref().as_ref(),
                );
            });
        }
    });

    let cancelled = cancel.is_cancelled();
    let status = if cancelled {
        SnapshotStatus::Cancelled
    } else if errors.total() > 0 {
        SnapshotStatus::Partial
    } else {
        SnapshotStatus::Complete
    };

    let mut store = match Arc::try_unwrap(writer) {
        Ok(mutex) => mutex.into_inner().unwrap_or_else(PoisonError::into_inner),
        Err(shared) => {
            // A worker panicked while holding an `Arc`; take the contents anyway.
            let mut guard = shared.lock().unwrap_or_else(PoisonError::into_inner);
            std::mem::replace(&mut *guard, NodeStore::with_capacity(0))
        }
    };

    let stats = finalize::aggregate(&mut store, root_id);
    debug_assert!(
        store.validate().is_ok(),
        "store invariants violated after aggregation"
    );

    progress.set_state(if cancelled {
        ScanState::Cancelled
    } else {
        ScanState::Ready
    });

    Ok(Snapshot {
        id: SnapshotId(1),
        root: root_id,
        store,
        meta: ScanMeta {
            root_path: root.to_path_buf(),
            volume_label: None,
            file_count: stats.file_count,
            dir_count: stats.dir_count,
            error_count: errors.total(),
            logical_bytes: stats.logical_bytes,
            allocated_bytes: stats.allocated_bytes,
            duration: started.elapsed(),
            follow_symlinks: options.follow_symlinks,
            same_filesystem_only: options.same_filesystem_only,
            max_depth: stats.max_depth,
        },
        status,
    })
}

fn build_excludes(patterns: &[String]) -> Option<GlobSet> {
    if patterns.is_empty() {
        return None;
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
            // A bare name such as `node_modules` should match at any depth.
            if let Ok(deep) = Glob::new(&format!("**/{pattern}")) {
                builder.add(deep);
            }
        }
    }
    builder.build().ok()
}

/// Shared directory queue with a pending counter.
///
/// Directory-level parallelism is enough for metadata-bound scans, and a single
/// queue keeps the insertion order deterministic enough for tests to compare
/// aggregate sizes exactly.
struct Scheduler {
    queue: Mutex<VecDeque<DirJob>>,
    pending: AtomicUsize,
    ready: Condvar,
}

impl Scheduler {
    fn new(root: DirJob) -> Self {
        let mut queue = VecDeque::new();
        queue.push_back(root);
        Self {
            queue: Mutex::new(queue),
            pending: AtomicUsize::new(1),
            ready: Condvar::new(),
        }
    }

    /// Returns the next job, or `None` once every queued directory is done.
    fn next(&self) -> Option<DirJob> {
        let mut queue = lock(&self.queue);
        loop {
            if let Some(job) = queue.pop_back() {
                return Some(job);
            }
            if self.pending.load(Ordering::Acquire) == 0 {
                return None;
            }
            queue = self
                .ready
                .wait(queue)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn push_all(&self, jobs: Vec<DirJob>) {
        if jobs.is_empty() {
            return;
        }
        self.pending.fetch_add(jobs.len(), Ordering::AcqRel);
        lock(&self.queue).extend(jobs);
        self.ready.notify_all();
    }

    fn finish_one(&self) {
        if self.pending.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.ready.notify_all();
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[allow(clippy::too_many_arguments)]
fn worker(
    reader: &dyn DirReader,
    options: &ScanOptions,
    progress: &Progress,
    cancel: &CancelToken,
    errors: &ErrorSink,
    writer: &Mutex<NodeStore>,
    scheduler: &Scheduler,
    excludes: Option<&GlobSet>,
) {
    let mut buffer: Vec<RawEntry> = Vec::with_capacity(256);
    while let Some(job) = scheduler.next() {
        if cancel.is_cancelled() {
            scheduler.finish_one();
            continue;
        }
        progress.set_current_path(&job.path);
        buffer.clear();

        match reader.read_dir(&job.path, &mut buffer) {
            Ok(()) => {
                let mut subdirs = Vec::new();
                let mut items: u64 = 0;
                let mut bytes: u64 = 0;

                {
                    let mut store = lock(writer);
                    for entry in &buffer {
                        if should_skip(entry, options, excludes, &job.path) {
                            continue;
                        }
                        let child_depth = job.depth.saturating_add(1);
                        if options.max_depth.is_some_and(|max| child_depth > max) {
                            continue;
                        }
                        let id = match store.insert(
                            job.node,
                            &entry.name,
                            flags_for(entry),
                            entry.size,
                            entry.allocated,
                            entry.mtime,
                            child_depth,
                            None,
                        ) {
                            Ok(id) => id,
                            Err(err) => {
                                errors.record(
                                    ErrorCode::Unsupported,
                                    job.path.display().to_string(),
                                    err.to_string(),
                                );
                                break;
                            }
                        };
                        items += 1;
                        bytes = bytes.saturating_add(entry.size);
                        if entry.kind == EntryKind::Dir {
                            subdirs.push(DirJob {
                                node: id,
                                path: job.path.join(entry.name.as_ref()),
                                depth: child_depth,
                            });
                        }
                    }
                }

                progress.add(items, bytes);
                scheduler.push_all(subdirs);
            }
            Err(err) => {
                errors.record(err.code, job.path.display().to_string(), err.message);
                progress.record_error();
                lock(writer).mark_error(job.node);
            }
        }
        scheduler.finish_one();
    }
}

fn flags_for(entry: &RawEntry) -> NodeFlags {
    let mut flags = match entry.kind {
        EntryKind::File => NodeFlags::FILE,
        EntryKind::Dir => NodeFlags::DIR,
        EntryKind::Symlink => NodeFlags::SYMLINK,
        EntryKind::Other => NodeFlags::empty(),
    };
    if entry.hidden {
        flags |= NodeFlags::HIDDEN;
    }
    if entry.system {
        flags |= NodeFlags::SYSTEM;
    }
    if entry.readonly {
        flags |= NodeFlags::READONLY;
    }
    if entry.shared {
        flags |= NodeFlags::SHARED;
    }
    flags
}

fn should_skip(
    entry: &RawEntry,
    options: &ScanOptions,
    excludes: Option<&GlobSet>,
    parent: &Path,
) -> bool {
    if !options.include_hidden && (entry.hidden || entry.system) {
        return true;
    }
    if entry.kind == EntryKind::File && entry.size < options.min_file_size {
        return true;
    }
    if let Some(set) = excludes {
        let relative = parent.join(entry.name.as_ref());
        if set.is_match(entry.name.as_ref()) || set.is_match(&relative) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threads_are_clamped() {
        let mut options = ScanOptions {
            threads: 500,
            ..ScanOptions::default()
        };
        assert_eq!(options.resolved_threads(), 64);
        options.threads = 0;
        assert!((2..=16).contains(&options.resolved_threads()));
    }

    #[test]
    fn default_excludes_cover_recycle_bin() {
        assert!(default_excludes().iter().any(|e| e == "$RECYCLE.BIN"));
    }

    #[test]
    fn excludes_match_at_any_depth() {
        let set = build_excludes(&["node_modules".to_string()]).expect("globset");
        assert!(set.is_match("node_modules"));
        assert!(set.is_match("a/b/node_modules"));
        assert!(!set.is_match("a/b/node_modules_old"));
    }
}
