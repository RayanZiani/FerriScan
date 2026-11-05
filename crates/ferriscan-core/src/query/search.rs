//! Bounded substring search over a snapshot.

use std::path::PathBuf;

use crate::model::{NodeId, Snapshot};
use crate::query::filters::contains_fold;

/// One search result: the node, its display name and its reconstructed path.
#[derive(Clone, Debug)]
pub struct SearchHit {
    /// Matching node.
    pub node: NodeId,
    /// Entry name.
    pub name: String,
    /// Path relative to the scan root.
    pub relative_path: PathBuf,
    /// Size in bytes.
    pub size: u64,
    /// Whether the entry is a directory.
    pub is_dir: bool,
}

/// Maximum number of hits returned to the UI.
pub const MAX_HITS: usize = 1_000;

/// Searches for `query` in every node name.
///
/// The scan is linear and bounded: it stops after [`MAX_HITS`] matches. A 1M-entry
/// snapshot is searched in well under the 80 ms budget because names are
/// contiguous in the arena and the comparison avoids allocation for ASCII.
#[must_use]
pub fn search(snapshot: &Snapshot, query: &str, limit: usize) -> Vec<SearchHit> {
    let limit = limit.clamp(1, MAX_HITS);
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }

    let store = &snapshot.store;
    let mut hits = Vec::with_capacity(limit.min(64));

    for (id, rec) in store.iter() {
        if id == snapshot.root {
            continue;
        }
        let name = store.name(id);
        if !contains_fold(name, query) {
            continue;
        }
        hits.push(SearchHit {
            node: id,
            name: name.to_owned(),
            relative_path: store.relative_path(id),
            size: rec.size,
            is_dir: rec.flags().is_dir(),
        });
        if hits.len() == limit {
            break;
        }
    }

    hits.sort_by(|a, b| {
        b.size
            .cmp(&a.size)
            .then_with(|| a.relative_path.cmp(&b.relative_path))
    });
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NodeFlags, NodeStore, ScanMeta, SnapshotId, SnapshotStatus};

    fn snapshot() -> Snapshot {
        let mut store = NodeStore::with_capacity(8);
        let root = store.insert_root("root", 0, NodeFlags::DIR).expect("root");
        store
            .insert(root, "rapport.pdf", NodeFlags::FILE, 10, 10, 0, 1, None)
            .expect("a");
        store
            .insert(root, "notes.txt", NodeFlags::FILE, 90, 90, 0, 1, None)
            .expect("b");
        store
            .insert(root, "Rapport.docx", NodeFlags::FILE, 40, 40, 0, 1, None)
            .expect("c");
        crate::scan::finalize::aggregate(&mut store, root);
        Snapshot {
            id: SnapshotId(1),
            root,
            store,
            meta: ScanMeta::for_root(PathBuf::from("/root")),
            status: SnapshotStatus::Complete,
        }
    }

    #[test]
    fn finds_case_insensitive_matches_sorted_by_size() {
        let snap = snapshot();
        let hits = search(&snap, "RAPPORT", 10);
        let names: Vec<_> = hits.iter().map(|h| h.name.as_str()).collect();
        assert_eq!(names, vec!["Rapport.docx", "rapport.pdf"]);
    }

    #[test]
    fn empty_query_returns_nothing() {
        let snap = snapshot();
        assert!(search(&snap, "   ", 10).is_empty());
    }

    #[test]
    fn results_are_bounded() {
        let snap = snapshot();
        assert!(search(&snap, "r", 1).len() <= 1);
    }
}
