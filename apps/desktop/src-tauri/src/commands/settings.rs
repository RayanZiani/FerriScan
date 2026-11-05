//! Settings commands.

use ferriscan_persist::{Settings, ThemeMode, Units};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

/// Settings payload exchanged with the frontend.
///
/// Mirrors the persisted struct but stays a separate type so the on-disk schema
/// can evolve without breaking the UI contract.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    /// `system`, `light` or `dark`.
    pub theme: String,
    /// BCP-47 tag, or `null` to follow the OS.
    pub language: Option<String>,
    /// `binary` or `decimal`.
    pub units: String,
    /// Compact table rows.
    pub compact_density: bool,
    /// Include hidden and system entries.
    pub include_hidden: bool,
    /// Stop at the filesystem boundary.
    pub same_filesystem_only: bool,
    /// Worker threads, `0` for automatic.
    pub threads: u16,
    /// Exclude globs.
    pub excludes: Vec<String>,
    /// Ask before a multi-item delete.
    pub confirm_multi_delete: bool,
    /// Ask before a permanent delete.
    pub confirm_permanent_delete: bool,
    /// Keep the optional cache between runs.
    pub keep_cache: bool,
    /// Log full paths (diagnostics only).
    pub verbose_logging: bool,
}

impl From<&Settings> for SettingsDto {
    fn from(settings: &Settings) -> Self {
        Self {
            theme: match settings.theme {
                ThemeMode::System => "system",
                ThemeMode::Light => "light",
                ThemeMode::Dark => "dark",
            }
            .to_string(),
            language: settings.language.clone(),
            units: match settings.units {
                Units::Binary => "binary",
                Units::Decimal => "decimal",
            }
            .to_string(),
            compact_density: settings.compact_density,
            include_hidden: settings.include_hidden,
            same_filesystem_only: settings.same_filesystem_only,
            threads: settings.threads,
            excludes: settings.excludes.clone(),
            confirm_multi_delete: settings.confirm_multi_delete,
            confirm_permanent_delete: settings.confirm_permanent_delete,
            keep_cache: settings.keep_cache,
            verbose_logging: settings.verbose_logging,
        }
    }
}

impl SettingsDto {
    /// Applies the payload onto a full settings value, preserving unknown keys.
    fn apply_to(&self, settings: &mut Settings) {
        settings.theme = match self.theme.as_str() {
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            _ => ThemeMode::System,
        };
        settings.language.clone_from(&self.language);
        if settings.language.as_deref() == Some("") {
            settings.language = None;
        }
        settings.units = if self.units == "decimal" {
            Units::Decimal
        } else {
            Units::Binary
        };
        settings.compact_density = self.compact_density;
        settings.include_hidden = self.include_hidden;
        settings.same_filesystem_only = self.same_filesystem_only;
        settings.threads = self.threads;
        settings.excludes.clone_from(&self.excludes);
        settings.confirm_multi_delete = self.confirm_multi_delete;
        settings.confirm_permanent_delete = self.confirm_permanent_delete;
        settings.keep_cache = self.keep_cache;
        settings.verbose_logging = self.verbose_logging;
    }
}

/// Returns the current settings.
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<SettingsDto, String> {
    let settings = state.settings();
    Ok(SettingsDto::from(&settings))
}

/// Persists new settings and returns the stored value.
#[tauri::command]
pub async fn set_settings(
    state: State<'_, AppState>,
    settings: SettingsDto,
) -> Result<SettingsDto, String> {
    let mut next = state.settings();
    settings.apply_to(&mut next);
    state.replace_settings(next)?;
    Ok(SettingsDto::from(&state.settings()))
}
