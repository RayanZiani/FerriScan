//! Read-only queries over the published snapshot.

use ferriscan_core::dto::{
    ChildrenPageDto, NodeDto, SnapshotMetaDto, SortDirectionDto, SortKeyDto, TreemapDto,
};
use ferriscan_core::layout::treemap::{TreemapRequest, treemap};
use ferriscan_core::model::{NodeId, SizeBasis};
use ferriscan_core::query::children::{SortDirection, SortKey, children_page};
use ferriscan_core::query::filters::FilterSet;
use tauri::State;

use crate::state::AppState;

/// Header of the current snapshot, or `null` when nothing has been scanned.
#[tauri::command]
pub async fn get_snapshot_meta(
    state: State<'_, AppState>,
) -> Result<Option<SnapshotMetaDto>, String> {
    Ok(state
        .current()
        .map(|snapshot| SnapshotMetaDto::from_snapshot(&snapshot)))
}

/// One node by id.
#[tauri::command]
pub async fn get_node(state: State<'_, AppState>, node_id: u32) -> Result<NodeDto, String> {
    let snapshot = state.current().ok_or("E_INVALID: no snapshot")?;
    NodeDto::from_snapshot(&snapshot, NodeId::from_raw(node_id))
        .ok_or_else(|| "E_INVALID: unknown node".to_string())
}

/// One page of children, sorted and filtered.
#[tauri::command]
pub async fn get_children(
    state: State<'_, AppState>,
    node_id: u32,
    sort: SortKeyDto,
    direction: SortDirectionDto,
    offset: u32,
    limit: u32,
) -> Result<ChildrenPageDto, String> {
    let snapshot = state.current().ok_or("E_INVALID: no snapshot")?;
    let node = NodeId::from_raw(node_id);
    if node.raw() as usize >= snapshot.store.len() {
        return Err("E_INVALID: unknown node".to_string());
    }
    let filters = FilterSet::default();
    let page = children_page(
        &snapshot,
        node,
        SortKey::from(sort),
        SortDirection::from(direction),
        &filters,
        offset,
        limit,
    );
    Ok(ChildrenPageDto::from_page(&snapshot, &page))
}

/// Precomputed treemap rectangles for one subtree.
#[tauri::command]
pub async fn get_treemap(
    state: State<'_, AppState>,
    node_id: u32,
    max_depth: u16,
    budget: usize,
    width: f32,
    height: f32,
) -> Result<TreemapDto, String> {
    let snapshot = state.current().ok_or("E_INVALID: no snapshot")?;
    let node = NodeId::from_raw(node_id);
    if node.raw() as usize >= snapshot.store.len() {
        return Err("E_INVALID: unknown node".to_string());
    }
    let tiles = treemap(
        &snapshot,
        TreemapRequest {
            node,
            max_depth,
            budget,
            width,
            height,
            basis: SizeBasis::Allocated,
        },
    );
    Ok(TreemapDto::from_tiles(&tiles))
}
