//! The flat node index: append-only during a scan, immutable once published.

use std::path::PathBuf;

use crate::model::extensions::{ExtensionIndex, extension_of};
use crate::model::node::{NONE, NodeFlags, NodeId, NodeRecord};

/// Hard ceiling on the number of nodes in one snapshot.
///
/// `NodeId` is a `u32` with `u32::MAX` reserved for `NONE`, so the store can hold
/// one fewer than 2^32 entries.
pub const MAX_NODES: usize = (u32::MAX - 1) as usize;

/// Why a node could not be inserted.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The snapshot reached [`MAX_NODES`].
    #[error("node capacity exhausted at {0} entries")]
    Capacity(usize),
    /// The name arena exceeded the `u32` offset space.
    #[error("name arena exceeded 4 GiB")]
    NameArenaFull,
    /// A name longer than `u16::MAX` bytes cannot be represented.
    #[error("name is {0} bytes, the maximum is {max}", max = u16::MAX)]
    NameTooLong(usize),
}

/// Flat, cache-friendly index of every entry in a snapshot.
///
/// Records live in one `Vec`; names live in a separate byte arena. Children are
/// linked through `first_child`/`next_sibling` so a parent never owns a `Vec`.
#[derive(Debug)]
pub struct NodeStore {
    nodes: Vec<NodeRecord>,
    names: Vec<u8>,
    extensions: ExtensionIndex,
    root: NodeId,
}

impl NodeStore {
    /// Creates a store with a capacity hint. The hint is clamped to [`MAX_NODES`].
    #[must_use]
    pub fn with_capacity(entries: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(entries.min(MAX_NODES)),
            names: Vec::with_capacity(entries.saturating_mul(16).min(64 << 20)),
            extensions: ExtensionIndex::new(),
            root: NodeId::NONE,
        }
    }

    /// Number of nodes currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Bytes used by the node records (excludes the name arena).
    #[must_use]
    pub fn records_bytes(&self) -> usize {
        self.nodes.capacity() * std::mem::size_of::<NodeRecord>()
    }

    /// Bytes used by the name arena.
    #[must_use]
    pub fn names_bytes(&self) -> usize {
        self.names.capacity()
    }

    /// Bytes used per indexed entry, excluding names.
    #[must_use]
    pub fn bytes_per_entry(&self) -> f64 {
        if self.nodes.is_empty() {
            0.0
        } else {
            std::mem::size_of::<NodeRecord>() as f64
        }
    }

    /// The root node, once the scan has started.
    #[must_use]
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Extension dictionary.
    #[must_use]
    pub fn extensions(&self) -> &ExtensionIndex {
        &self.extensions
    }

    /// Borrows a record. Panics only on an invalid id, which is a snapshot bug.
    #[must_use]
    pub fn get(&self, id: NodeId) -> &NodeRecord {
        &self.nodes[id.raw() as usize]
    }

    /// Name of a node.
    #[must_use]
    pub fn name(&self, id: NodeId) -> &str {
        let rec = self.get(id);
        let start = rec.name_off as usize;
        let end = start + usize::from(rec.name_len);
        // SAFETY of the unwrap-equivalent: names are inserted from `&str` and the
        // offset is derived from the same insertion, so the slice is valid UTF-8.
        std::str::from_utf8(&self.names[start..end]).unwrap_or("")
    }

    /// Children of `id`, in insertion order.
    pub fn children(&self, id: NodeId) -> ChildrenIter<'_> {
        ChildrenIter {
            store: self,
            next: self.get(id).first_child,
        }
    }

    /// Path of a node, reconstructed by walking parents.
    ///
    /// The returned path is relative to the scan root and is built on demand;
    /// the engine deliberately stores no paths.
    #[must_use]
    pub fn relative_path(&self, id: NodeId) -> PathBuf {
        let mut parts = Vec::with_capacity(8);
        let mut cur = id;
        while !cur.is_none() && cur != self.root {
            parts.push(self.name(cur));
            cur = self.get(cur).parent();
        }
        let mut path = PathBuf::new();
        for part in parts.iter().rev() {
            path.push(part);
        }
        path
    }

    /// Inserts the root node and returns its id.
    pub fn insert_root(
        &mut self,
        name: &str,
        mtime: i64,
        flags: NodeFlags,
    ) -> Result<NodeId, StoreError> {
        let id = self.insert(NodeId::NONE, name, flags, 0, 0, mtime, 0, Some(0))?;
        self.root = id;
        Ok(id)
    }

    /// Appends a node under `parent`.
    ///
    /// This is the hottest path in the scanner: it performs one arena copy, one
    /// possible growth, and three link updates. No allocation happens when the
    /// capacity hint was right.
    #[allow(clippy::too_many_arguments)]
    pub fn insert(
        &mut self,
        parent: NodeId,
        name: &str,
        flags: NodeFlags,
        size: u64,
        allocated: u64,
        mtime: i64,
        depth: u16,
        ext: Option<u16>,
    ) -> Result<NodeId, StoreError> {
        if self.nodes.len() >= MAX_NODES {
            return Err(StoreError::Capacity(self.nodes.len()));
        }
        let name_len =
            u16::try_from(name.len()).map_err(|_| StoreError::NameTooLong(name.len()))?;
        let name_off = u32::try_from(self.names.len()).map_err(|_| StoreError::NameArenaFull)?;
        self.names.extend_from_slice(name.as_bytes());

        let idx = self.nodes.len();
        // The cast is safe: `idx < MAX_NODES <= u32::MAX`.
        let raw = u32::try_from(idx).unwrap_or(NONE);
        let ext_id = ext.unwrap_or_else(|| {
            if flags.is_file() {
                self.extensions.intern(extension_of(name))
            } else {
                0
            }
        });

        self.nodes.push(NodeRecord {
            size,
            allocated,
            mtime,
            name_off,
            parent: parent.raw(),
            first_child: NONE,
            last_child: NONE,
            next_sibling: NONE,
            child_count: 0,
            name_len,
            ext_id,
            depth,
            flags: flags.bits(),
        });

        if !parent.is_none() {
            let parent_idx = parent.raw() as usize;
            let last = self.nodes[parent_idx].last_child;
            if last == NONE {
                self.nodes[parent_idx].first_child = raw;
            } else {
                self.nodes[last as usize].next_sibling = raw;
            }
            self.nodes[parent_idx].last_child = raw;
            self.nodes[parent_idx].child_count += 1;
        }

        Ok(NodeId::from_raw(raw))
    }

    /// Overwrites the aggregated size of a node (used by `finalize`).
    pub fn set_size(&mut self, id: NodeId, size: u64, allocated: u64) {
        let rec = &mut self.nodes[id.raw() as usize];
        rec.size = size;
        rec.allocated = allocated;
    }

    /// Marks a node as partially read.
    pub fn mark_error(&mut self, id: NodeId) {
        self.nodes[id.raw() as usize].flags |= NodeFlags::ERROR.bits();
    }
    /// Iterates every record with its id, in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &NodeRecord)> {
        self.nodes
            .iter()
            .enumerate()
            .map(|(i, rec)| (NodeId::from_raw(u32::try_from(i).unwrap_or(NONE)), rec))
    }

    /// Iterates nodes in reverse insertion order.
    ///
    /// Children are always inserted after their parent, so a reverse pass visits
    /// every child before its parent: that is the post-order aggregation used by
    /// `finalize`, without recursion.
    pub fn iter_rev(&self) -> impl Iterator<Item = (NodeId, &NodeRecord)> {
        self.nodes
            .iter()
            .enumerate()
            .rev()
            .map(|(i, rec)| (NodeId::from_raw(u32::try_from(i).unwrap_or(NONE)), rec))
    }

    /// Debug-only structural validation; used by tests and `debug_assert`s.
    pub fn validate(&self) -> Result<(), String> {
        if self.nodes.len() > MAX_NODES {
            return Err("store exceeds MAX_NODES".into());
        }
        let mut seen = vec![false; self.nodes.len()];
        for (i, rec) in self.nodes.iter().enumerate() {
            let end = rec.name_off as usize + usize::from(rec.name_len);
            if end > self.names.len() {
                return Err(format!("node {i} name out of arena bounds"));
            }
            if std::str::from_utf8(&self.names[rec.name_off as usize..end]).is_err() {
                return Err(format!("node {i} name is not UTF-8"));
            }
            let mut child = rec.first_child;
            let mut count = 0u32;
            while child != NONE {
                let ci = child as usize;
                if ci >= self.nodes.len() {
                    return Err(format!("node {i} child {ci} out of range"));
                }
                if seen[ci] {
                    return Err(format!("node {ci} reached twice"));
                }
                seen[ci] = true;
                if self.nodes[ci].parent != u32::try_from(i).unwrap_or(NONE) {
                    return Err(format!("node {ci} parent pointer mismatch"));
                }
                if self.nodes[ci].depth != rec.depth.saturating_add(1) {
                    return Err(format!("node {ci} depth mismatch"));
                }
                count += 1;
                child = self.nodes[ci].next_sibling;
            }
            if count != rec.child_count {
                return Err(format!("node {i} child_count mismatch"));
            }
        }
        Ok(())
    }
}

/// Iterator over a node's direct children.
#[derive(Debug)]
pub struct ChildrenIter<'a> {
    store: &'a NodeStore,
    next: u32,
}

impl Iterator for ChildrenIter<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next == NONE {
            return None;
        }
        let id = NodeId::from_raw(self.next);
        self.next = self.store.get(id).next_sibling;
        Some(id)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.next == NONE {
            (0, Some(0))
        } else {
            (0, None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_store() -> (NodeStore, NodeId) {
        let mut store = NodeStore::with_capacity(8);
        let root = store
            .insert_root("root", 0, NodeFlags::DIR)
            .expect("root insert");
        let a = store
            .insert(root, "a.txt", NodeFlags::FILE, 100, 100, 0, 1, None)
            .expect("file a");
        store
            .insert(root, "sub", NodeFlags::DIR, 0, 0, 0, 1, None)
            .expect("dir sub");
        let _ = a;
        (store, root)
    }

    #[test]
    fn inserts_and_links_children() {
        let (store, root) = small_store();
        assert_eq!(store.len(), 3);
        assert_eq!(store.get(root).child_count(), 2);
        let names: Vec<_> = store.children(root).map(|id| store.name(id)).collect();
        assert_eq!(names, vec!["a.txt", "sub"]);
        assert_eq!(
            store.relative_path(NodeId::from_raw(1)),
            PathBuf::from("a.txt")
        );
        store.validate().expect("valid store");
    }

    #[test]
    fn interns_extension_on_insert() {
        let (store, _) = small_store();
        let rec = store.get(NodeId::from_raw(1));
        assert_eq!(store.extensions().name(rec.ext_id), "txt");
    }

    #[test]
    fn rejects_overlong_names() {
        let mut store = NodeStore::with_capacity(2);
        let root = store.insert_root("root", 0, NodeFlags::DIR).expect("root");
        let long = "x".repeat(usize::from(u16::MAX) + 1);
        let err = store
            .insert(root, &long, NodeFlags::FILE, 0, 0, 0, 1, None)
            .expect_err("must fail");
        assert!(matches!(err, StoreError::NameTooLong(_)));
    }

    #[test]
    fn reverse_iteration_visits_children_first() {
        let (store, root) = small_store();
        let order: Vec<_> = store.iter_rev().map(|(id, _)| id).collect();
        assert_eq!(order.last().copied(), Some(root));
    }
}
