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

    /// Row index one step up or down from `from` (the first row when nothing is selected),
    /// clamped to the ends of the list; `None` when the folder is empty.
    #[must_use]
    pub fn step(&self, from: Option<NodeId>, down: bool) -> Option<usize> {
        let last = self.rows.len().checked_sub(1)?;
        let at = from.and_then(|id| self.rows.iter().position(|row| *row == id));
        Some(match (at, down) {
            (None, _) => 0,
            (Some(i), true) => (i + 1).min(last),
            (Some(i), false) => i.saturating_sub(1),
        })
    }

    /// Whether the view shows the scan root.
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

    #[test]
    fn arrow_steps_start_at_the_top_and_stop_at_the_ends() {
        let r = Results::new(snapshot());
        let (first, second) = (r.rows[0], r.rows[1]);
        assert_eq!(r.step(None, true), Some(0));
        assert_eq!(r.step(None, false), Some(0));
        assert_eq!(r.step(Some(first), true), Some(1));
        assert_eq!(r.step(Some(second), true), Some(1));
        assert_eq!(r.step(Some(second), false), Some(0));
        assert_eq!(r.step(Some(first), false), Some(0));
    }
}
