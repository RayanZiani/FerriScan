//! Scanning commands and progress reporting.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ferriscan_core::dto::{ScanOptionsDto, ScanProgressDto, ScanRequestDto, ScanStateDto};
use ferriscan_core::model::SnapshotStatus;
use ferriscan_core::scan::progress::ScanState;
use ferriscan_core::scan::{ScanOptions, scan};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::state::AppState;

/// Starts a scan on a background thread and returns immediately.
///
/// Results arrive through `scan://progress` and `scan://done`; `get_scan_progress`
/// and `get_snapshot_meta` are the fallbacks after a webview reload.
#[tauri::command]
pub async fn start_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ScanRequestDto,
) -> Result<(), String> {
    if !state.begin_scan() {
        return Err("E_BUSY: a scan is already running".into());
    }

    state.cancel.reset();
    state.progress.reset();

    let progress = Arc::clone(&state.progress);
    let cancel = Arc::clone(&state.cancel);
    let options = to_options(&request.options);
    let path = std::path::PathBuf::from(&request.path);
    let handle = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let reader = ferriscan_platform::PlatformDirReader::new();

        let reporter = {
            let progress = Arc::clone(&progress);
            let handle = handle.clone();
            std::thread::spawn(move || report_loop(&progress, &handle, started))
        };

        let result = scan(&reader, &path, &options, &progress, &cancel);

        progress.set_state(match &result {
            Ok(snapshot) if snapshot.status == SnapshotStatus::Cancelled => ScanState::Cancelled,
            Ok(_) => ScanState::Ready,
            Err(_) => ScanState::Failed,
        });
        let _ = reporter.join();

        let state = handle.state::<AppState>();
        match result {
            Ok(snapshot) => {
                let meta = ferriscan_core::dto::SnapshotMetaDto::from_snapshot(&snapshot);
                state.publish(snapshot);
                let _ = handle.emit("scan://done", meta);
            }
            Err(err) => {
                let _ = handle.emit("scan://error", err.to_string());
            }
        }
        state.end_scan();
    });

    Ok(())
}

/// Requests cancellation of the running scan. Idempotent.
#[tauri::command]
pub async fn cancel_scan(state: State<'_, AppState>) -> Result<(), String> {
    state.cancel.cancel();
    state.progress.set_state(ScanState::Cancelling);
    Ok(())
}

/// Returns the last known progress, used after a webview reload.
#[tauri::command]
pub async fn get_scan_progress(state: State<'_, AppState>) -> Result<ScanProgressDto, String> {
    let snap = state.progress.snapshot();
    // A reloaded webview can miss the terminal event; report `scanning` when the
    // worker is still alive so the UI does not show a stale ready state.
    let state_value = if state.is_scanning() && ScanState::from_u8(snap.state).is_terminal() {
        ScanState::Scanning
    } else {
        ScanState::from_u8(snap.state)
    };
    Ok(ScanProgressDto {
        state: ScanStateDto::from(state_value),
        items: snap.items,
        bytes: snap.bytes,
        errors: snap.errors,
        current_path: snap.current_path,
        elapsed_ms: 0,
        items_per_second: 0,
    })
}

fn report_loop(
    progress: &ferriscan_core::scan::progress::Progress,
    app: &AppHandle,
    started: Instant,
) {
    loop {
        std::thread::sleep(Duration::from_millis(100));
        let snap = progress.snapshot();
        let state = ScanState::from_u8(snap.state);
        if state.is_terminal() || state == ScanState::Cancelling {
            break;
        }
        let elapsed = started.elapsed().as_secs_f64().max(0.001);
        let payload = ScanProgressDto {
            state: ScanStateDto::from(state),
            items: snap.items,
            bytes: snap.bytes,
            errors: snap.errors,
            current_path: snap.current_path,
            elapsed_ms: (elapsed * 1000.0) as u64,
            items_per_second: (snap.items as f64 / elapsed) as u64,
        };
        let _ = app.emit("scan://progress", payload);
    }
}

fn to_options(dto: &ScanOptionsDto) -> ScanOptions {
    ScanOptions {
        include_hidden: dto.include_hidden,
        same_filesystem_only: dto.same_filesystem_only,
        min_file_size: dto.min_file_size,
        max_depth: dto.max_depth,
        threads: usize::from(dto.threads),
        excludes: dto.excludes.clone(),
        follow_symlinks: dto.follow_symlinks,
    }
}
