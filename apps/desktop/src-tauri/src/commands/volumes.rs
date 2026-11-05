//! Volume listing.

use ferriscan_core::dto::{VolumeDto, VolumeKind};
use ferriscan_platform::volumes::Volume;
use tauri::State;

use crate::state::AppState;

/// Lists every mounted volume with capacity and free space.
///
/// Reads the OS on each call; the UI polls only while the volume list is visible.
#[tauri::command]
pub async fn list_volumes(state: State<'_, AppState>) -> Result<Vec<VolumeDto>, String> {
    let dirs = state.dirs.clone();
    let volumes = tauri::async_runtime::spawn_blocking(move || {
        let _ = dirs.ensure();
        ferriscan_platform::list_volumes()
    })
    .await
    .map_err(|e| format!("volume enumeration failed: {e}"))?;

    Ok(volumes.into_iter().map(to_dto).collect())
}

fn to_dto(volume: Volume) -> VolumeDto {
    let used = volume.total_bytes.saturating_sub(volume.available_bytes);
    VolumeDto {
        id: volume.mount_point.display().to_string(),
        mount_point: volume.mount_point.display().to_string(),
        label: volume.label,
        filesystem: volume.filesystem,
        kind: match volume.kind {
            ferriscan_platform::volumes::VolumeKind::Fixed => VolumeKind::Fixed,
            ferriscan_platform::volumes::VolumeKind::Removable => VolumeKind::Removable,
            ferriscan_platform::volumes::VolumeKind::Network => VolumeKind::Network,
            ferriscan_platform::volumes::VolumeKind::Optical => VolumeKind::Optical,
            ferriscan_platform::volumes::VolumeKind::Ram => VolumeKind::Ram,
            ferriscan_platform::volumes::VolumeKind::Unknown => VolumeKind::Unknown,
        },
        total_bytes: volume.total_bytes,
        available_bytes: volume.available_bytes,
        used_bytes: used,
        read_only: volume.read_only,
        writable: !volume.read_only && !volume.locked,
        locked: volume.locked,
    }
}
