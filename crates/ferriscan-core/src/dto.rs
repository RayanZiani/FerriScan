//! IPC data-transfer objects.
//!
//! Every structure that crosses the Tauri boundary is defined here once and
//! mirrored into TypeScript by `xtask contract`. Field names are `camelCase` on
//! the wire, sizes are integers, and optional values are explicit `null`.

use serde::{Deserialize, Serialize};

use crate::layout::treemap::TreemapTile;
use crate::model::{NodeFlags, NodeId, Snapshot, SnapshotStatus};
use crate::query::children::{ChildrenPage, SortDirection, SortKey};
use crate::query::search::SearchHit;

/// Volume kind, as reported by the OS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VolumeKind {
    /// Internal fixed disk.
    Fixed,
    /// USB stick or memory card.
    Removable,
    /// Network share.
    Network,
    /// CD/DVD/Blu-ray.
    Optical,
    /// RAM disk.
    Ram,
    /// Anything else.
    Unknown,
}

/// One mounted volume.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeDto {
    /// Stable id for the mount point.
    pub id: String,
    /// Mount point, e.g. `C:\` or `/home`.
    pub mount_point: String,
    /// Human label, e.g. `Windows`.
    pub label: String,
    /// Filesystem name, e.g. `NTFS`, `ext4`, `apfs`.
    pub filesystem: String,
    /// Kind.
    pub kind: VolumeKind,
    /// Capacity in bytes.
    pub total_bytes: u64,
    /// Free bytes.
    pub available_bytes: u64,
    /// Bytes shown as used by the OS (capacity minus free minus reserved).
    pub used_bytes: u64,
    /// Whether the volume is mounted read-only.
    pub read_only: bool,
    /// Whether the current user can write to the root.
    pub writable: bool,
    /// Whether the volume is locked (`BitLocker`) and cannot be read.
    pub locked: bool,
}

/// Scan lifecycle state on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScanStateDto {
    /// Nothing running.
    Idle,
    /// Options validated.
    Preparing,
    /// Enumerating.
    Scanning,
    /// Aggregating.
    Finalizing,
    /// Snapshot available.
    Ready,
    /// Cancelling.
    Cancelling,
    /// Cancelled with partial results.
    Cancelled,
    /// Failed.
    Failed,
}

impl From<crate::scan::progress::ScanState> for ScanStateDto {
    fn from(value: crate::scan::progress::ScanState) -> Self {
        use crate::scan::progress::ScanState;
        match value {
            ScanState::Idle => Self::Idle,
            ScanState::Preparing => Self::Preparing,
            ScanState::Scanning => Self::Scanning,
            ScanState::Finalizing => Self::Finalizing,
            ScanState::Ready => Self::Ready,
            ScanState::Cancelling => Self::Cancelling,
            ScanState::Cancelled => Self::Cancelled,
            ScanState::Failed => Self::Failed,
        }
    }
}

/// Options accepted by `start_scan`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOptionsDto {
    /// Include hidden and system entries.
    pub include_hidden: bool,
    /// Stop at the filesystem boundary.
    pub same_filesystem_only: bool,
    /// Skip files below this size.
    pub min_file_size: u64,
    /// Maximum depth, `null` for unlimited.
    pub max_depth: Option<u16>,
    /// Worker threads, `0` for automatic.
    pub threads: u16,
    /// Exclude globs.
    pub excludes: Vec<String>,
    /// Whether the platform reader may follow links.
    pub follow_symlinks: bool,
}

/// Request for `start_scan`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRequestDto {
    /// Absolute path to scan.
    pub path: String,
    /// Options.
    pub options: ScanOptionsDto,
}

/// Progress event payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgressDto {
    /// Lifecycle state.
    pub state: ScanStateDto,
    /// Entries indexed.
    pub items: u64,
    /// Logical bytes seen.
    pub bytes: u64,
    /// Errors.
    pub errors: u64,
    /// Current directory, already shortened for display.
    pub current_path: Option<String>,
    /// Elapsed milliseconds.
    pub elapsed_ms: u64,
    /// Entries per second, smoothed.
    pub items_per_second: u64,
}

/// Snapshot header shown in the status bar.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotMetaDto {
    /// Hex snapshot id.
    pub id: String,
    /// Root path.
    pub root_path: String,
    /// Display name.
    pub root_name: String,
    /// Node id of the scan root, used to seed navigation.
    pub root_node_id: u32,
    /// `complete`, `partial` or `cancelled`.
    pub status: &'static str,
    /// Files indexed.
    pub file_count: u64,
    /// Directories indexed.
    pub dir_count: u64,
    /// Entries that failed.
    pub error_count: u64,
    /// Logical bytes.
    pub logical_bytes: u64,
    /// Allocated bytes.
    pub allocated_bytes: u64,
    /// Scan duration in milliseconds.
    pub duration_ms: u64,
    /// Deepest level reached.
    pub max_depth: u16,
}

impl SnapshotMetaDto {
    /// Builds the header from a finished snapshot.
    #[must_use]
    pub fn from_snapshot(snapshot: &Snapshot) -> Self {
        let status = match snapshot.status {
            SnapshotStatus::Complete => "complete",
            SnapshotStatus::Partial => "partial",
            SnapshotStatus::Cancelled => "cancelled",
        };
        Self {
            id: snapshot.id.to_string(),
            root_path: snapshot.meta.root_path.display().to_string(),
            root_name: snapshot.root_name().to_owned(),
            root_node_id: snapshot.root.raw(),
            status,
            file_count: snapshot.meta.file_count,
            dir_count: snapshot.meta.dir_count,
            error_count: snapshot.meta.error_count,
            logical_bytes: snapshot.meta.logical_bytes,
            allocated_bytes: snapshot.meta.allocated_bytes,
            duration_ms: u64::try_from(snapshot.meta.duration.as_millis()).unwrap_or(u64::MAX),
            max_depth: snapshot.meta.max_depth,
        }
    }
}

/// A single node for the inspector and table rows.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDto {
    /// Node id.
    pub id: u32,
    /// Entry name.
    pub name: String,
    /// Parent id, or `null` for the root.
    pub parent: Option<u32>,
    /// Logical size.
    pub size: u64,
    /// Allocated size.
    pub allocated: u64,
    /// Modification time, Unix seconds.
    pub mtime: i64,
    /// Direct child count.
    pub child_count: u32,
    /// Depth relative to the scan root.
    pub depth: u16,
    /// Is a directory.
    pub is_dir: bool,
    /// Is a file.
    pub is_file: bool,
    /// Is a symlink.
    pub is_symlink: bool,
    /// Hidden attribute.
    pub hidden: bool,
    /// Error attribute.
    pub has_error: bool,
    /// Interned extension, empty when none.
    pub extension: String,
    /// Path relative to the scan root.
    pub relative_path: String,
}

impl NodeDto {
    /// Builds a DTO from the snapshot.
    #[must_use]
    pub fn from_snapshot(snapshot: &Snapshot, id: NodeId) -> Option<Self> {
        let store = &snapshot.store;
        if id.is_none() || id.raw() as usize >= store.len() {
            return None;
        }
        let rec = store.get(id);
        let flags: NodeFlags = rec.flags();
        Some(Self {
            id: id.raw(),
            name: store.name(id).to_owned(),
            parent: if rec.parent().is_none() {
                None
            } else {
                Some(rec.parent().raw())
            },
            size: rec.size,
            allocated: rec.allocated,
            mtime: rec.mtime,
            child_count: rec.child_count(),
            depth: rec.depth,
            is_dir: flags.is_dir(),
            is_file: flags.is_file(),
            is_symlink: flags.is_symlink(),
            hidden: flags.is_hidden(),
            has_error: flags.contains(NodeFlags::ERROR),
            extension: store.extensions().name(rec.ext_id).to_owned(),
            relative_path: store.relative_path(id).display().to_string(),
        })
    }
}

/// Sort key on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortKeyDto {
    /// Size.
    Size,
    /// Name.
    Name,
    /// Item count.
    Items,
    /// Modification date.
    Modified,
    /// Extension.
    Type,
}

impl From<SortKeyDto> for SortKey {
    fn from(value: SortKeyDto) -> Self {
        match value {
            SortKeyDto::Size => Self::Size,
            SortKeyDto::Name => Self::Name,
            SortKeyDto::Items => Self::Items,
            SortKeyDto::Modified => Self::Modified,
            SortKeyDto::Type => Self::Type,
        }
    }
}

/// Sort direction on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortDirectionDto {
    /// Ascending.
    Ascending,
    /// Descending.
    Descending,
}

impl From<SortDirectionDto> for SortDirection {
    fn from(value: SortDirectionDto) -> Self {
        match value {
            SortDirectionDto::Ascending => Self::Ascending,
            SortDirectionDto::Descending => Self::Descending,
        }
    }
}

/// A page of children.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildrenPageDto {
    /// Parent id.
    pub parent: u32,
    /// Rows.
    pub rows: Vec<NodeDto>,
    /// Total matching children.
    pub total: u32,
    /// Offset of the first row.
    pub offset: u32,
}

impl ChildrenPageDto {
    /// Builds a page DTO, skipping ids that no longer resolve.
    #[must_use]
    pub fn from_page(snapshot: &Snapshot, page: &ChildrenPage) -> Self {
        let rows = page
            .ids
            .iter()
            .filter_map(|id| NodeDto::from_snapshot(snapshot, *id))
            .collect();
        Self {
            parent: page.parent.raw(),
            rows,
            total: page.total_matching,
            offset: page.offset,
        }
    }
}

/// One search result on the wire.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitDto {
    /// Node id.
    pub id: u32,
    /// Entry name.
    pub name: String,
    /// Relative path.
    pub path: String,
    /// Size.
    pub size: u64,
    /// Is a directory.
    pub is_dir: bool,
}

impl From<&SearchHit> for SearchHitDto {
    fn from(hit: &SearchHit) -> Self {
        Self {
            id: hit.node.raw(),
            name: hit.name.clone(),
            path: hit.relative_path.display().to_string(),
            size: hit.size,
            is_dir: hit.is_dir,
        }
    }
}

/// A treemap tile on the wire, using flat arrays to keep the payload small.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreemapDto {
    /// `u32` node ids aligned with the geometry arrays; `NONE` marks aggregates.
    pub nodes: Vec<u32>,
    /// Left edges.
    pub xs: Vec<f32>,
    /// Top edges.
    pub ys: Vec<f32>,
    /// Widths.
    pub ws: Vec<f32>,
    /// Heights.
    pub hs: Vec<f32>,
    /// Depths relative to the root tile.
    pub depths: Vec<u16>,
    /// Sizes in bytes that produced each rectangle.
    pub sizes: Vec<u64>,
}

impl TreemapDto {
    /// Flattens tiles into the columnar representation.
    #[must_use]
    pub fn from_tiles(tiles: &[TreemapTile]) -> Self {
        let mut dto = Self {
            nodes: Vec::with_capacity(tiles.len()),
            xs: Vec::with_capacity(tiles.len()),
            ys: Vec::with_capacity(tiles.len()),
            ws: Vec::with_capacity(tiles.len()),
            hs: Vec::with_capacity(tiles.len()),
            depths: Vec::with_capacity(tiles.len()),
            sizes: Vec::with_capacity(tiles.len()),
        };
        for tile in tiles {
            dto.nodes.push(tile.node.raw());
            dto.xs.push(tile.rect.x);
            dto.ys.push(tile.rect.y);
            dto.ws.push(tile.rect.w);
            dto.hs.push(tile.rect.h);
            dto.depths.push(tile.depth);
            dto.sizes.push(tile.size);
        }
        dto
    }
}

/// Structured error returned to the frontend.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorDto {
    /// Stable code such as `E_PERM`.
    pub code: String,
    /// Non-localized technical message.
    pub message: String,
    /// Whether retrying might succeed.
    pub retryable: bool,
}

impl AppErrorDto {
    /// Builds an error DTO from a code and message.
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_dto_of_root_has_no_parent() {
        let mut store = crate::model::NodeStore::with_capacity(4);
        let root = store.insert_root("root", 0, NodeFlags::DIR).expect("root");
        crate::scan::finalize::aggregate(&mut store, root);
        let snap = Snapshot {
            id: crate::model::SnapshotId(1),
            root,
            store,
            meta: crate::model::ScanMeta::for_root(std::path::PathBuf::from("/root")),
            status: SnapshotStatus::Complete,
        };
        let dto = NodeDto::from_snapshot(&snap, root).expect("dto");
        assert_eq!(dto.parent, None);
        assert!(dto.is_dir);
    }

    #[test]
    fn unknown_node_yields_none() {
        let mut store = crate::model::NodeStore::with_capacity(2);
        let root = store.insert_root("root", 0, NodeFlags::DIR).expect("root");
        let snap = Snapshot {
            id: crate::model::SnapshotId(1),
            root,
            store,
            meta: crate::model::ScanMeta::for_root(std::path::PathBuf::from("/root")),
            status: SnapshotStatus::Complete,
        };
        assert!(NodeDto::from_snapshot(&snap, NodeId::from_raw(99)).is_none());
        assert!(NodeDto::from_snapshot(&snap, NodeId::NONE).is_none());
    }
}
