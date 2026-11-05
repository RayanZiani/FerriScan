//! Treemap layout over a snapshot subtree.

use crate::layout::squarify::{Rect, squarify};
use crate::model::{NodeId, SizeBasis, Snapshot};

/// Default rectangle budget; matches the spec's 5,000-node frame target.
pub const DEFAULT_BUDGET: usize = 5_000;
/// Default depth limit below the requested root.
pub const DEFAULT_MAX_DEPTH: u16 = 8;

/// Parameters for one treemap request.
#[derive(Clone, Copy, Debug)]
pub struct TreemapRequest {
    /// Node to lay out.
    pub node: NodeId,
    /// Depth below `node` to descend.
    pub max_depth: u16,
    /// Maximum number of tiles, including aggregated "more" tiles.
    pub budget: usize,
    /// Width of the drawing area in logical pixels.
    pub width: f32,
    /// Height of the drawing area in logical pixels.
    pub height: f32,
    /// Which size to weight rectangles by.
    pub basis: SizeBasis,
}

/// One rendered tile.
#[derive(Clone, Copy, Debug)]
pub struct TreemapTile {
    /// Node this tile represents, or `NONE` for an aggregated remainder.
    pub node: NodeId,
    /// Parent tile, or `NONE` for the root tile.
    pub parent: NodeId,
    /// Depth relative to the requested root (root tile is 0).
    pub depth: u16,
    /// Geometry in layout space.
    pub rect: Rect,
    /// Weight that produced the geometry, in bytes.
    pub size: u64,
}

/// Computes a treemap for `request`.
///
/// Children smaller than the visible threshold are folded into a synthetic
/// `+N more` tile so the result stays within budget and no 1-pixel rectangles
/// reach the renderer.
#[must_use]
pub fn treemap(snapshot: &Snapshot, request: TreemapRequest) -> Vec<TreemapTile> {
    let budget = request.budget.clamp(16, 20_000);
    let area = Rect {
        x: 0.0,
        y: 0.0,
        w: request.width.max(1.0),
        h: request.height.max(1.0),
    };

    let root_size = size_of(snapshot, request.node, request.basis);
    if root_size == 0 {
        return Vec::new();
    }

    let mut tiles = vec![TreemapTile {
        node: request.node,
        parent: NodeId::NONE,
        depth: 0,
        rect: area,
        size: root_size,
    }];

    let mut cursor = 0usize;
    while cursor < tiles.len() && tiles.len() < budget {
        let tile = tiles[cursor];
        cursor += 1;
        if tile.depth >= request.max_depth || !tile.rect.is_visible() {
            continue;
        }

        let mut children: Vec<(NodeId, u64)> = snapshot
            .store
            .children(tile.node)
            .map(|id| (id, size_of(snapshot, id, request.basis)))
            .filter(|(_, size)| *size > 0)
            .collect();
        if children.is_empty() {
            continue;
        }
        children.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let room = budget.saturating_sub(tiles.len());
        let (visible, remainder) = if children.len() > room {
            let split = room.saturating_sub(1);
            (children[..split].to_vec(), Some(children[split..].to_vec()))
        } else {
            (children, None)
        };

        let mut weights: Vec<f64> = visible.iter().map(|(_, size)| *size as f64).collect();
        if let Some(rest) = &remainder {
            let sum: u64 = rest.iter().map(|(_, s)| *s).sum();
            weights.push(sum as f64);
        }

        let rects = squarify(&weights, tile.rect);
        for (i, rect) in rects.iter().enumerate() {
            if !rect.is_visible() {
                continue;
            }
            if i < visible.len() {
                let (node, size) = visible[i];
                tiles.push(TreemapTile {
                    node,
                    parent: tile.node,
                    depth: tile.depth.saturating_add(1),
                    rect: *rect,
                    size,
                });
            } else if let Some(rest) = &remainder {
                tiles.push(TreemapTile {
                    node: NodeId::NONE,
                    parent: tile.node,
                    depth: tile.depth.saturating_add(1),
                    rect: *rect,
                    size: rest.iter().map(|(_, s)| *s).sum(),
                });
            }
        }
    }

    tiles
}

fn size_of(snapshot: &Snapshot, node: NodeId, basis: SizeBasis) -> u64 {
    let rec = snapshot.store.get(node);
    match basis {
        SizeBasis::Allocated => rec
            .allocated
            .max(if rec.flags().is_file() { rec.size } else { 0 }),
        SizeBasis::Logical => rec.size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NodeFlags, NodeStore, ScanMeta, SnapshotId, SnapshotStatus};

    fn snapshot() -> Snapshot {
        let mut store = NodeStore::with_capacity(64);
        let root = store.insert_root("root", 0, NodeFlags::DIR).expect("root");
        for i in 0..10u64 {
            let dir = store
                .insert(root, &format!("d{i}"), NodeFlags::DIR, 0, 0, 0, 1, Some(0))
                .expect("dir");
            for j in 0..5u64 {
                store
                    .insert(
                        dir,
                        &format!("f{j}.bin"),
                        NodeFlags::FILE,
                        (i + 1) * 10 + j,
                        (i + 1) * 10 + j,
                        0,
                        2,
                        None,
                    )
                    .expect("file");
            }
        }
        crate::scan::finalize::aggregate(&mut store, root);
        Snapshot {
            id: SnapshotId(1),
            root,
            store,
            meta: ScanMeta::for_root(std::path::PathBuf::from("/root")),
            status: SnapshotStatus::Complete,
        }
    }

    fn request(snap: &Snapshot) -> TreemapRequest {
        TreemapRequest {
            node: snap.root,
            max_depth: 3,
            budget: 100,
            width: 800.0,
            height: 600.0,
            basis: SizeBasis::Logical,
        }
    }

    #[test]
    fn root_tile_covers_the_area() {
        let snap = snapshot();
        let tiles = treemap(&snap, request(&snap));
        assert!(!tiles.is_empty());
        assert_eq!(tiles[0].depth, 0);
        assert!((tiles[0].rect.w - 800.0).abs() < 0.1);
    }

    #[test]
    fn respects_the_budget() {
        let snap = snapshot();
        let mut r = request(&snap);
        r.budget = 20;
        assert!(treemap(&snap, r).len() <= 20);
    }

    #[test]
    fn layout_is_deterministic() {
        let snap = snapshot();
        let a = treemap(&snap, request(&snap));
        let b = treemap(&snap, request(&snap));
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.node, y.node);
            assert_eq!(x.rect, y.rect);
        }
    }

    #[test]
    fn empty_subtree_yields_no_tiles() {
        let mut store = NodeStore::with_capacity(2);
        let root = store.insert_root("root", 0, NodeFlags::DIR).expect("root");
        let snap = Snapshot {
            id: SnapshotId(1),
            root,
            store,
            meta: ScanMeta::for_root(std::path::PathBuf::from("/root")),
            status: SnapshotStatus::Complete,
        };
        assert!(treemap(&snap, request(&snap)).is_empty());
    }
}
