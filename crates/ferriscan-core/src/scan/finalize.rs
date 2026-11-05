//! Post-scan aggregation.
//!
//! Children are always inserted after their parent, so a single reverse pass
//! accumulates every subtree without recursion and without a stack: for each
//! node, its total is added to its parent's accumulator.

use crate::model::{NodeId, NodeStore};

/// Totals computed by [`aggregate`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregateStats {
    /// Files indexed.
    pub file_count: u64,
    /// Directories indexed (including the root).
    pub dir_count: u64,
    /// Sum of logical bytes.
    pub logical_bytes: u64,
    /// Sum of allocated bytes.
    pub allocated_bytes: u64,
    /// Deepest level reached.
    pub max_depth: u16,
}

/// Computes directory sizes, counts and depth in one reverse pass.
///
/// Files keep their own size. Directories are overwritten with the sum of their
/// subtree, which the sibling-chain invariant then guarantees is consistent.
pub fn aggregate(store: &mut NodeStore, root: NodeId) -> AggregateStats {
    let len = store.len();
    let mut totals_size = vec![0u64; len];
    let mut totals_alloc = vec![0u64; len];
    let mut is_dir = vec![false; len];
    let mut parent = vec![0u32; len];
    let mut depth = vec![0u16; len];
    let mut stats = AggregateStats::default();

    for (id, rec) in store.iter() {
        let idx = id.raw() as usize;
        totals_size[idx] = rec.size;
        totals_alloc[idx] = rec.allocated;
        is_dir[idx] = rec.flags().is_dir();
        parent[idx] = rec.parent;
        depth[idx] = rec.depth;
        if rec.flags().is_dir() {
            stats.dir_count += 1;
        } else if rec.flags().is_file() {
            stats.file_count += 1;
            stats.logical_bytes = stats.logical_bytes.saturating_add(rec.size);
            stats.allocated_bytes = stats.allocated_bytes.saturating_add(rec.allocated);
        }
        stats.max_depth = stats.max_depth.max(rec.depth);
    }

    for idx in (0..len).rev() {
        let total_size = totals_size[idx];
        let total_alloc = totals_alloc[idx];
        let parent_raw = parent[idx];
        if parent_raw != crate::model::NONE {
            let p = parent_raw as usize;
            totals_size[p] = totals_size[p].saturating_add(total_size);
            totals_alloc[p] = totals_alloc[p].saturating_add(total_alloc);
        }
        if is_dir[idx] {
            let id = NodeId::from_raw(u32::try_from(idx).unwrap_or(0));
            store.set_size(id, total_size, total_alloc);
        }
    }

    let root_idx = root.raw() as usize;
    debug_assert!(root_idx < len, "root must be inside the store");
    stats.logical_bytes = totals_size[root_idx];
    stats.allocated_bytes = totals_alloc[root_idx];
    let _ = depth;
    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NodeFlags;

    #[test]
    fn directories_sum_their_subtree() {
        let mut store = NodeStore::with_capacity(16);
        let root = store.insert_root("root", 0, NodeFlags::DIR).expect("root");
        let sub = store
            .insert(root, "sub", NodeFlags::DIR, 0, 0, 0, 1, Some(0))
            .expect("sub");
        store
            .insert(sub, "a.bin", NodeFlags::FILE, 100, 128, 0, 2, None)
            .expect("a");
        store
            .insert(root, "b.bin", NodeFlags::FILE, 40, 64, 0, 1, None)
            .expect("b");

        let stats = aggregate(&mut store, root);
        assert_eq!(store.get(sub).size, 100);
        assert_eq!(store.get(sub).allocated, 128);
        assert_eq!(store.get(root).size, 140);
        assert_eq!(store.get(root).allocated, 192);
        assert_eq!(stats.logical_bytes, 140);
        assert_eq!(stats.allocated_bytes, 192);
        assert_eq!(stats.file_count, 2);
        assert_eq!(stats.dir_count, 2);
        assert_eq!(stats.max_depth, 2);
        store.validate().expect("valid after aggregation");
    }

    #[test]
    fn empty_directory_is_zero() {
        let mut store = NodeStore::with_capacity(4);
        let root = store.insert_root("root", 0, NodeFlags::DIR).expect("root");
        let stats = aggregate(&mut store, root);
        assert_eq!(store.get(root).size, 0);
        assert_eq!(stats.logical_bytes, 0);
        assert_eq!(stats.dir_count, 1);
    }
}
