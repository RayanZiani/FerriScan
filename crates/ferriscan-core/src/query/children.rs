//! Paged, sorted and filtered child listing.

use crate::model::{NodeId, NodeStore, Snapshot};
use crate::query::filters::FilterSet;

/// Column a listing can be sorted by.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SortKey {
    /// Sort by the active size basis.
    #[default]
    Size,
    /// Sort by name, case-insensitively.
    Name,
    /// Sort by number of direct children.
    Items,
    /// Sort by modification time.
    Modified,
    /// Sort by extension, empty names last.
    Type,
}

/// Sort direction.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SortDirection {
    /// Largest or Z first.
    #[default]
    Descending,
    /// Smallest or A first.
    Ascending,
}

impl SortDirection {
    fn is_ascending(self) -> bool {
        matches!(self, Self::Ascending)
    }
}

/// One page of children plus the total number of matches before paging.
#[derive(Clone, Debug)]
pub struct ChildrenPage {
    /// Node these children belong to.
    pub parent: NodeId,
    /// Ids on this page, already sorted.
    pub ids: Vec<NodeId>,
    /// Number of children matching the filters, across all pages.
    pub total_matching: u32,
    /// Offset this page starts at.
    pub offset: u32,
}

/// Maximum rows a single page may request.
pub const MAX_PAGE_SIZE: u32 = 500;

/// Returns one page of the children of `parent`.
///
/// The implementation collects and sorts the matching children of one directory.
/// Directories with pathological child counts (hundreds of thousands) are rare
/// and are bounded by the 10 ms budget in the spec; a sorted index can be added
/// later without changing this signature (ADR-0002 follow-up).
#[must_use]
pub fn children_page(
    snapshot: &Snapshot,
    parent: NodeId,
    sort: SortKey,
    direction: SortDirection,
    filters: &FilterSet,
    offset: u32,
    limit: u32,
) -> ChildrenPage {
    let store = &snapshot.store;
    let limit = limit.clamp(1, MAX_PAGE_SIZE);

    let mut ids: Vec<NodeId> = store
        .children(parent)
        .filter(|id| filters.matches(store, *id))
        .collect();

    sort_ids(store, &mut ids, sort, direction);

    let total_matching = u32::try_from(ids.len()).unwrap_or(u32::MAX);
    let offset = offset.min(total_matching);
    let start = offset as usize;
    let end = (start + limit as usize).min(ids.len());
    let page = ids
        .get(start..end)
        .map_or_else(Vec::new, <[NodeId]>::to_vec);

    ChildrenPage {
        parent,
        ids: page,
        total_matching,
        offset,
    }
}

fn sort_ids(store: &NodeStore, ids: &mut [NodeId], key: SortKey, direction: SortDirection) {
    let asc = direction.is_ascending();
    ids.sort_by(|a, b| {
        let ra = store.get(*a);
        let rb = store.get(*b);
        let ord = match key {
            SortKey::Size => ra.size.cmp(&rb.size),
            SortKey::Items => ra.child_count.cmp(&rb.child_count),
            SortKey::Modified => ra.mtime.cmp(&rb.mtime),
            SortKey::Name => cmp_name(store.name(*a), store.name(*b)),
            SortKey::Type => {
                let ea = store.extensions().name(ra.ext_id);
                let eb = store.extensions().name(rb.ext_id);
                cmp_name(ea, eb)
            }
        };
        // Ties fall back to name, then id, so the order is fully deterministic.
        let ord = ord.then_with(|| cmp_name(store.name(*a), store.name(*b)));
        let ord = ord.then_with(|| a.raw().cmp(&b.raw()));
        if asc { ord } else { ord.reverse() }
    });
}

/// Case-insensitive byte comparison; good enough for the C locale and stable
/// across platforms, unlike locale-aware collation.
fn cmp_name(a: &str, b: &str) -> std::cmp::Ordering {
    let mut ai = a.bytes().map(|b| b.to_ascii_lowercase());
    let mut bi = b.bytes().map(|b| b.to_ascii_lowercase());
    loop {
        match (ai.next(), bi.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x != y => return x.cmp(&y),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NodeFlags;

    fn snapshot() -> Snapshot {
        let mut store = NodeStore::with_capacity(8);
        let root = store.insert_root("root", 0, NodeFlags::DIR).expect("root");
        store
            .insert(root, "b.txt", NodeFlags::FILE, 10, 10, 5, 1, None)
            .expect("b");
        store
            .insert(root, "a.bin", NodeFlags::FILE, 30, 30, 6, 1, None)
            .expect("a");
        store
            .insert(root, "c", NodeFlags::DIR, 0, 0, 7, 1, Some(0))
            .expect("c");
        crate::scan::finalize::aggregate(&mut store, root);
        Snapshot {
            id: crate::model::SnapshotId(1),
            root,
            store,
            meta: crate::model::ScanMeta::for_root(std::path::PathBuf::from("/root")),
            status: crate::model::SnapshotStatus::Complete,
        }
    }

    #[test]
    fn sorts_by_size_descending() {
        let snap = snapshot();
        let page = children_page(
            &snap,
            snap.root,
            SortKey::Size,
            SortDirection::Descending,
            &FilterSet::default(),
            0,
            10,
        );
        let names: Vec<_> = page.ids.iter().map(|id| snap.store.name(*id)).collect();
        assert_eq!(names, vec!["a.bin", "b.txt", "c"]);
        assert_eq!(page.total_matching, 3);
    }

    #[test]
    fn sorts_by_name_ascending() {
        let snap = snapshot();
        let page = children_page(
            &snap,
            snap.root,
            SortKey::Name,
            SortDirection::Ascending,
            &FilterSet::default(),
            0,
            10,
        );
        let names: Vec<_> = page.ids.iter().map(|id| snap.store.name(*id)).collect();
        assert_eq!(names, vec!["a.bin", "b.txt", "c"]);
    }

    #[test]
    fn paging_slices_without_reordering() {
        let snap = snapshot();
        let first = children_page(
            &snap,
            snap.root,
            SortKey::Name,
            SortDirection::Ascending,
            &FilterSet::default(),
            0,
            2,
        );
        let second = children_page(
            &snap,
            snap.root,
            SortKey::Name,
            SortDirection::Ascending,
            &FilterSet::default(),
            2,
            2,
        );
        assert_eq!(first.ids.len(), 2);
        assert_eq!(second.ids.len(), 1);
        assert_eq!(second.total_matching, 3);
    }

    #[test]
    fn limit_is_capped() {
        let snap = snapshot();
        let page = children_page(
            &snap,
            snap.root,
            SortKey::Size,
            SortDirection::Descending,
            &FilterSet::default(),
            0,
            10_000,
        );
        assert!(page.ids.len() <= MAX_PAGE_SIZE as usize);
    }
}
