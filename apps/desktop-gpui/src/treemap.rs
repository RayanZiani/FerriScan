//! Treemap layout cache and hit-testing shared by paint and mouse handling.

use ferriscan_core::layout::treemap::{
    DEFAULT_BUDGET, DEFAULT_MAX_DEPTH, TreemapRequest, TreemapTile, treemap,
};
use ferriscan_core::model::{NodeId, SizeBasis, Snapshot};
use gpui::{Bounds, Pixels};

/// Tiles for the folder currently shown, recomputed only when folder or size changes.
pub struct TreemapState {
    key: (u32, u32, u32),
    pub tiles: Vec<TreemapTile>,
    /// Window-space bounds recorded at the last paint.
    pub bounds: Bounds<Pixels>,
}

impl Default for TreemapState {
    fn default() -> Self {
        Self {
            key: (0, 0, u32::MAX),
            tiles: Vec::new(),
            bounds: Bounds::default(),
        }
    }
}

impl TreemapState {
    /// Lays out `node` for a `w` x `h` area unless the cached layout already matches.
    pub fn ensure(&mut self, snapshot: &Snapshot, node: NodeId, w: f32, h: f32) {
        let key = (w.to_bits(), h.to_bits(), node.raw());
        if self.key == key {
            return;
        }
        self.tiles = treemap(
            snapshot,
            TreemapRequest {
                node,
                max_depth: DEFAULT_MAX_DEPTH,
                budget: DEFAULT_BUDGET,
                width: w,
                height: h,
                basis: SizeBasis::Allocated,
            },
        );
        self.key = key;
    }

    /// Deepest tile containing the layout-space point; tiles are parent-first, so the last
    /// match is the innermost one.
    #[must_use]
    pub fn hit(&self, x: f32, y: f32) -> Option<usize> {
        self.tiles.iter().rposition(|t| {
            let r = t.rect;
            x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h
        })
    }
}

#[cfg(test)]
mod tests {
    use ferriscan_core::layout::squarify::Rect;

    use super::*;

    fn tile(depth: u16, x: f32, w: f32) -> TreemapTile {
        TreemapTile {
            node: NodeId::from_raw(depth.into()),
            parent: NodeId::NONE,
            depth,
            rect: Rect {
                x,
                y: 0.0,
                w,
                h: 10.0,
            },
            size: 1,
        }
    }

    #[test]
    fn hit_prefers_the_innermost_tile() {
        let state = TreemapState {
            tiles: vec![tile(0, 0.0, 100.0), tile(1, 0.0, 40.0)],
            ..TreemapState::default()
        };
        assert_eq!(state.hit(10.0, 5.0), Some(1));
        assert_eq!(state.hit(60.0, 5.0), Some(0));
        assert_eq!(state.hit(60.0, 50.0), None);
    }
}
