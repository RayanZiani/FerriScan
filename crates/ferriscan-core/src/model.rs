//! Model layer: nodes, the flat store, snapshots and extension interning.

mod extensions;
mod node;
mod snapshot;
mod store;

pub use extensions::{ExtensionIndex, extension_of};
pub use node::{NONE, NodeFlags, NodeId, NodeRecord};
pub use snapshot::{ScanMeta, SizeBasis, Snapshot, SnapshotId, SnapshotStatus};
pub use store::{ChildrenIter, MAX_NODES, NodeStore, StoreError};
