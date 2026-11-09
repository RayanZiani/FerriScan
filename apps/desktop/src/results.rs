//! Folder-by-folder browsing state over one immutable snapshot.

use std::{path::PathBuf, sync::Arc};

use ferriscan_core::model::{NodeId, Snapshot};

/// Current folder of a finished scan and its children, largest first.
pub struct Results {
    pub snapshot: Arc<Snapshot>,
    pub current: NodeId,
    pub rows: Vec<NodeId>,
}

impl Results {
    #[must_use]
    pub fn new(snapshot: Arc<Snapshot>) -> Self {
        let root = snapshot.root;
        let mut results = Self {
            snapshot,
            current: root,
            rows: Vec::new(),
        };
        results.load(root);
        results
    }

    /// Enters `id` if it is a directory.
    pub fn open(&mut self, id: NodeId) {
        if self.snapshot.store.get(id).flags().is_dir() {
            self.load(id);
        }
    }

    /// Goes to the parent folder; no-op at the scan root.
    pub fn up(&mut self) {
        if self.current != self.snapshot.root {
            let parent = NodeId::from_raw(self.snapshot.store.get(self.current).parent);
            self.load(parent);
        }
    }

    #[must_use]
    pub fn at_root(&self) -> bool {
        self.current == self.snapshot.root
    }

    /// Absolute path of the current folder, for display only.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.snapshot
            .meta
            .root_path
            .join(self.snapshot.store.relative_path(self.current))
    }

    fn load(&mut self, folder: NodeId) {
        let store = &self.snapshot.store;
        let mut rows: Vec<NodeId> = store.children(folder).collect();
        // Name tiebreak keeps the order reproducible across runs (AGENTS principle 10).
        rows.sort_by(|a, b| {
            store
                .get(*b)
                .size
                .cmp(&store.get(*a).size)
                .then_with(|| store.name(*a).cmp(store.name(*b)))
        });
        self.rows = rows;
        self.current = folder;
    }
}

#[cfg(test)]
mod tests {
    use ferriscan_core::model::{NodeFlags, NodeStore, ScanMeta, SnapshotId, SnapshotStatus};

    use super::*;

    fn snapshot() -> Arc<Snapshot> {
        let mut store = NodeStore::with_capacity(8);
        let root = store.insert_root("root", 0, NodeFlags::DIR).unwrap();
        let add = |store: &mut NodeStore, parent, name, flags, size| {
            store
                .insert(parent, name, flags, size, size, 0, 1, None)
                .unwrap()
        };
        let dir = add(&mut store, root, "dir", NodeFlags::DIR, 100);
        add(&mut store, root, "small", NodeFlags::FILE, 10);
        add(&mut store, dir, "big", NodeFlags::FILE, 100);
        Arc::new(Snapshot {
            id: SnapshotId(1),
            root,
            store,
            meta: ScanMeta::for_root(PathBuf::from("root")),
            status: SnapshotStatus::Complete,
        })
    }

    #[test]
    fn rows_are_largest_first_and_navigation_round_trips() {
        let mut r = Results::new(snapshot());
        let names: Vec<_> = r.rows.iter().map(|id| r.snapshot.store.name(*id)).collect();
        assert_eq!(names, ["dir", "small"]);

        r.open(r.rows[0]);
        assert!(!r.at_root());
        assert_eq!(r.rows.len(), 1);

        r.up();
        assert!(r.at_root());
        r.up(); // no-op at root
        assert!(r.at_root());
    }
}
