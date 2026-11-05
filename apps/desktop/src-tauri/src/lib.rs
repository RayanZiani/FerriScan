//! `FerriScan` desktop shell.
//!
//! Thin composition root: it owns the Tauri state, registers commands, and
//! forwards events. All real work lives in `ferriscan-core`,
//! `ferriscan-platform` and `ferriscan-persist`.

mod commands;
mod state;

use tauri::Manager;

/// Starts the application.
///
/// # Errors
///
/// Returns the Tauri runtime error if the window or event loop cannot start.
pub fn run() -> tauri::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .compact()
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state::AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::system::get_system_info,
            commands::volumes::list_volumes,
            commands::scan::start_scan,
            commands::scan::cancel_scan,
            commands::scan::get_scan_progress,
            commands::query::get_snapshot_meta,
            commands::query::get_node,
            commands::query::get_children,
            commands::query::get_treemap,
            commands::settings::get_settings,
            commands::settings::set_settings,
        ])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("FerriScan");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
}
