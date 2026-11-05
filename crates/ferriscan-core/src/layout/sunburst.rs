//! Sunburst layout: nested rings of arcs.

use crate::model::{NodeId, SizeBasis, Snapshot};

/// Default arc budget.
pub const DEFAULT_ARC_BUDGET: usize = 5_000;

/// Parameters for one sunburst request.
#[derive(Clone, Copy, Debug)]
pub struct SunburstRequest {
    /// Node at the center.
    pub node: NodeId,
    /// Number of rings below `node`.
    pub max_depth: u16,
    /// Maximum arcs, including aggregated ones.
    pub budget: usize,
    /// Which size to weight arcs by.
    pub basis: SizeBasis,
}

/// One arc, in radians measured clockwise from 12 o'clock.
#[derive(Clone, Copy, Debug)]
pub struct SunburstArc {
    /// Node represented, or `NONE` for an aggregated remainder.
    pub node: NodeId,
    /// Parent arc, or `NONE` for the center.
    pub parent: NodeId,
    /// Ring index; the center is ring 0.
    pub depth: u16,
    /// Start angle in radians, `0.0..=2π`.
    pub start: f32,
    /// End angle in radians, greater than `start`.
    pub end: f32,
    /// Weight that produced the arc.
    pub size: u64,
}

impl SunburstArc {
    /// Arc span in radians.
    #[must_use]
    pub fn span(&self) -> f32 {
        (self.end - self.start).max(0.0)
    }
}

/// Computes a sunburst layout for `request`.
#[must_use]
pub fn sunburst(snapshot: &Snapshot, request: SunburstRequest) -> Vec<SunburstArc> {
    let budget = request.budget.clamp(16, 20_000);
    let root_size = size_of(snapshot, request.node, request.basis);
    if root_size == 0 {
        return Vec::new();
    }

    let full_turn = std::f32::consts::TAU;
    let mut arcs = vec![SunburstArc {
        node: request.node,
        parent: NodeId::NONE,
        depth: 0,
        start: 0.0,
        end: full_turn,
        size: root_size,
    }];

    let mut cursor = 0usize;
    while cursor < arcs.len() && arcs.len() < budget {
        let parent = arcs[cursor];
        cursor += 1;
        if parent.depth >= request.max_depth {
            continue;
        }

        let mut children: Vec<(NodeId, u64)> = snapshot
            .store
            .children(parent.node)
            .map(|id| (id, size_of(snapshot, id, request.basis)))
            .filter(|(_, size)| *size > 0)
            .collect();
        if children.is_empty() {
            continue;
        }
        // Keep the visual order stable: biggest first, clockwise.
        children.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let room = budget.saturating_sub(arcs.len());
        let (visible, remainder) = if children.len() > room {
            let split = room.saturating_sub(1);
            (children[..split].to_vec(), Some(children[split..].to_vec()))
        } else {
            (children, None)
        };

        let total: u64 = visible.iter().map(|(_, s)| *s).sum::<u64>()
            + remainder
                .as_ref()
                .map_or(0, |r| r.iter().map(|(_, s)| *s).sum::<u64>());
        if total == 0 {
            continue;
        }

        let mut angle = parent.start;
        let span = parent.end - parent.start;
        for (node, size) in &visible {
            let fraction = *size as f32 / total as f32;
            let end = (angle + span * fraction).min(parent.end);
            arcs.push(SunburstArc {
                node: *node,
                parent: parent.node,
                depth: parent.depth.saturating_add(1),
                start: angle,
                end,
                size: *size,
            });
            angle = end;
        }
        if let Some(rest) = remainder {
            let sum: u64 = rest.iter().map(|(_, s)| *s).sum();
            arcs.push(SunburstArc {
                node: NodeId::NONE,
                parent: parent.node,
                depth: parent.depth.saturating_add(1),
                start: angle,
                end: parent.end,
                size: sum,
            });
        }
    }

    arcs
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
        let mut store = NodeStore::with_capacity(16);
        let root = store.insert_root("root", 0, NodeFlags::DIR).expect("root");
        for (name, size) in [("a", 50u64), ("b", 30), ("c", 20)] {
            store
                .insert(root, name, NodeFlags::FILE, size, size, 0, 1, None)
                .expect("file");
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

    #[test]
    fn children_span_the_full_turn() {
        let snap = snapshot();
        let arcs = sunburst(
            &snap,
            SunburstRequest {
                node: snap.root,
                max_depth: 2,
                budget: 100,
                basis: SizeBasis::Logical,
            },
        );
        let covered: f32 = arcs
            .iter()
            .filter(|a| a.depth == 1)
            .map(SunburstArc::span)
            .sum();
        assert!((covered - std::f32::consts::TAU).abs() < 0.01);
    }

    #[test]
    fn proportional_to_size() {
        let snap = snapshot();
        let arcs = sunburst(
            &snap,
            SunburstRequest {
                node: snap.root,
                max_depth: 1,
                budget: 100,
                basis: SizeBasis::Logical,
            },
        );
        let a = arcs
            .iter()
            .find(|a| a.depth == 1 && a.size == 50)
            .expect("a");
        assert!((a.span() - std::f32::consts::TAU * 0.5).abs() < 0.01);
    }

    #[test]
    fn respects_the_budget() {
        let snap = snapshot();
        let arcs = sunburst(
            &snap,
            SunburstRequest {
                node: snap.root,
                max_depth: 4,
                budget: 16,
                basis: SizeBasis::Logical,
            },
        );
        assert!(arcs.len() <= 16);
    }
}
