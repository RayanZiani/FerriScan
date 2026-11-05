//! System information: locale, theme and platform identity.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

/// Everything the frontend needs to render the first frame correctly.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfoDto {
    /// `windows`, `linux` or `macos`.
    pub platform: String,
    /// Application version from the crate metadata.
    pub app_version: String,
    /// Detected or overridden locale.
    pub locale: String,
    /// Whether the locale came from a saved user choice.
    pub locale_overridden: bool,
    /// Configured theme preference.
    pub theme: String,
    /// Whether the OS is in high-contrast mode (best effort).
    pub high_contrast: bool,
    /// Whether the OS asks for reduced motion (best effort).
    pub reduced_motion: bool,
    /// Settings file path, shown in About for transparency.
    pub settings_path: String,
}

/// Returns locale, theme and build information.
#[tauri::command]
pub async fn get_system_info(state: State<'_, AppState>) -> Result<SystemInfoDto, String> {
    let settings = state.settings();
    let locale = ferriscan_i18n::resolve(settings.language.as_deref());
    Ok(SystemInfoDto {
        platform: ferriscan_platform::platform_name().to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        locale,
        locale_overridden: settings.language.is_some(),
        theme: match settings.theme {
            ferriscan_persist::ThemeMode::System => "system",
            ferriscan_persist::ThemeMode::Light => "light",
            ferriscan_persist::ThemeMode::Dark => "dark",
        }
        .to_string(),
        high_contrast: false,
        reduced_motion: false,
        settings_path: state.dirs.settings_file().display().to_string(),
    })
}
