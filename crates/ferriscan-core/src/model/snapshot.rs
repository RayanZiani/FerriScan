//! Snapshot: the immutable result of one scan.

use std::path::PathBuf;
use std::time::Duration;

use crate::model::node::NodeId;
use crate::model::store::NodeStore;

/// Monotonic snapshot identifier, displayed as hex.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct SnapshotId(pub u64);

impl std::fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:x}", self.0)
    }
}

/// How complete a snapshot is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SnapshotStatus {
    /// Every reachable directory was read.
    Complete,
    /// Some directories failed; results are usable but incomplete.
    Partial,
    /// The user cancelled; results cover what was indexed at that point.
    Cancelled,
}

/// Which size is displayed and aggregated by default.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SizeBasis {
    /// Bytes actually used on disk.
    #[default]
    Allocated,
    /// Sum of file lengths.
    Logical,
}

/// Scan configuration captured with the snapshot, so a reopened result explains
/// how it was produced.
#[derive(Clone, Debug)]
pub struct ScanMeta {
    /// Root path that was scanned.
    pub root_path: PathBuf,
    /// Display name of the scanned volume, when the root is a mount point.
    pub volume_label: Option<String>,
    /// Number of files indexed.
    pub file_count: u64,
    /// Number of directories indexed.
    pub dir_count: u64,
    /// Number of entries that could not be read.
    pub error_count: u64,
    /// Total logical bytes.
    pub logical_bytes: u64,
    /// Total allocated bytes.
    pub allocated_bytes: u64,
    /// Wall-clock duration of the scan.
    pub duration: Duration,
    /// Whether symlinks were followed.
    pub follow_symlinks: bool,
    /// Whether the scan stopped at a filesystem boundary.
    pub same_filesystem_only: bool,
    /// Maximum depth reached.
    pub max_depth: u16,
}

impl ScanMeta {
    /// Empty metadata for a scan that has not finished.
    #[must_use]
    pub fn for_root(root_path: PathBuf) -> Self {
        Self {
            root_path,
            volume_label: None,
            file_count: 0,
            dir_count: 0,
            error_count: 0,
            logical_bytes: 0,
            allocated_bytes: 0,
            duration: Duration::ZERO,
            follow_symlinks: false,
            same_filesystem_only: false,
            max_depth: 0,
        }
    }
}

/// An immutable scan result, shared as `Arc<Snapshot>` with the UI and queries.
#[derive(Debug)]
pub struct Snapshot {
    /// Identifier, monotonic within the process.
    pub id: SnapshotId,
    /// Root of the tree.
    pub root: NodeId,
    /// The node index.
    pub store: NodeStore,
    /// Capture metadata.
    pub meta: ScanMeta,
    /// Completion state.
    pub status: SnapshotStatus,
}

impl Snapshot {
    /// Total size of the tree under the chosen basis.
    #[must_use]
    pub fn total_size(&self, basis: SizeBasis) -> u64 {
        match basis {
            SizeBasis::Allocated => self.meta.allocated_bytes,
            SizeBasis::Logical => self.meta.logical_bytes,
        }
    }

    /// Entry count (files plus directories).
    #[must_use]
    pub fn entry_count(&self) -> u64 {
        self.store.len() as u64
    }

    /// Name of the root node, used as a fallback display title.
    #[must_use]
    pub fn root_name(&self) -> &str {
        self.store.name(self.root)
    }
}
