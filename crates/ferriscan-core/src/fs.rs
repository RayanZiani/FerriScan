//! Filesystem abstraction consumed by the engine.
//!
//! `ferriscan-core` never calls the OS directly. A [`DirReader`] returns the raw
//! entries of one directory; everything above it (filtering, indexing, sizing)
//! is platform-independent and unit-testable.

mod entry;
mod fake;

pub use entry::{DirReader, EntryKind, FsError, RawEntry};
pub use fake::FakeFs;

use crate::model::NodeId;

/// One directory to enumerate during a scan.
#[derive(Clone, Debug)]
pub struct DirJob {
    /// Node representing the directory.
    pub node: NodeId,
    /// Absolute path to read.
    pub path: std::path::PathBuf,
    /// Depth of the directory itself.
    pub depth: u16,
    /// Reader token for the directory (see [`DirReader::read_dir_keyed`]).
    pub key: u64,
}
