//! Versioned, atomic, forward-compatible settings.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Theme preference.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
    /// Follow the operating system.
    #[default]
    System,
    /// Always light.
    Light,
    /// Always dark.
    Dark,
}

/// Byte unit convention.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Units {
    /// 1024-based (KiB, MiB).
    #[default]
    Binary,
    /// 1000-based (kB, MB).
    Decimal,
}

/// Default view when a scan finishes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DefaultView {
    /// Tree table.
    #[default]
    Table,
    /// Treemap.
    Treemap,
    /// Sunburst.
    Sunburst,
    /// Extension breakdown.
    Extensions,
    /// Largest files.
    Largest,
}

/// Every persisted user preference.
///
/// `schema` is bumped only for incompatible changes; additive fields keep the
/// same version and rely on `#[serde(default)]`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Schema version of this document.
    pub schema: u32,
    /// Light, dark or system.
    pub theme: ThemeMode,
    /// UI language tag; `None` follows the OS.
    pub language: Option<String>,
    /// Binary or decimal units.
    pub units: Units,
    /// Compact rows.
    pub compact_density: bool,
    /// View opened after a scan.
    pub default_view: DefaultView,
    /// Include hidden and system entries by default.
    pub include_hidden: bool,
    /// Stop at filesystem boundaries by default.
    pub same_filesystem_only: bool,
    /// Worker threads; `0` is automatic.
    pub threads: u16,
    /// Exclude globs applied to every scan.
    pub excludes: Vec<String>,
    /// Ask before deleting more than one item.
    pub confirm_multi_delete: bool,
    /// Ask before permanently deleting anything.
    pub confirm_permanent_delete: bool,
    /// Keep the optional snapshot cache between runs.
    pub keep_cache: bool,
    /// Log full paths (off by default: privacy).
    pub verbose_logging: bool,
    /// Window geometry: x, y, width, height.
    pub window_geometry: Option<(i32, i32, u32, u32)>,
    /// Column widths for the tree table, keyed by column id.
    pub column_widths: BTreeMap<String, u32>,
    /// Keys written by a newer version that this build does not understand.
    #[serde(skip)]
    pub unknown: BTreeMap<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: CURRENT_SCHEMA,
            theme: ThemeMode::System,
            language: None,
            units: Units::Binary,
            compact_density: false,
            default_view: DefaultView::Table,
            include_hidden: true,
            same_filesystem_only: false,
            threads: 0,
            excludes: ferriscan_core::scan::default_excludes(),
            confirm_multi_delete: true,
            confirm_permanent_delete: true,
            keep_cache: false,
            verbose_logging: false,
            window_geometry: None,
            column_widths: BTreeMap::new(),
            unknown: BTreeMap::new(),
        }
    }
}

/// Current schema version.
pub const CURRENT_SCHEMA: u32 = 1;

impl Settings {
    /// Loads settings, returning defaults when the file is missing or unreadable.
    ///
    /// A corrupt file is never fatal: defaults are returned and the original file
    /// is left untouched so the user can inspect it.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        match serde_json::from_str::<Self>(&raw) {
            Ok(mut settings) => {
                settings.capture_unknown(&raw);
                settings
            }
            Err(err) => {
                tracing::warn!(error = %err, "settings file is invalid; using defaults");
                Self::default()
            }
        }
    }

    /// Serializes to pretty JSON, preserving unknown keys.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let mut value = serde_json::to_value(self)?;
        if let serde_json::Value::Object(map) = &mut value {
            for (key, unknown) in &self.unknown {
                map.entry(key.clone()).or_insert_with(|| unknown.clone());
            }
        }
        serde_json::to_string_pretty(&value)
    }

    /// Writes atomically: temp file in the same directory, then rename.
    pub fn save(&self, path: &Path) -> Result<(), SaveError> {
        let json = self.to_json().map_err(SaveError::Serialize)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(SaveError::Io)?;
        }
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, json).map_err(SaveError::Io)?;
        std::fs::rename(&temp, path).map_err(SaveError::Io)?;
        Ok(())
    }

    fn capture_unknown(&mut self, raw: &str) {
        let Ok(serde_json::Value::Object(map)) = serde_json::from_str(raw) else {
            return;
        };
        let known = [
            "schema",
            "theme",
            "language",
            "units",
            "compactDensity",
            "defaultView",
            "includeHidden",
            "sameFilesystemOnly",
            "threads",
            "excludes",
            "confirmMultiDelete",
            "confirmPermanentDelete",
            "keepCache",
            "verboseLogging",
            "windowGeometry",
            "columnWidths",
        ];
        for (key, value) in map {
            if !known.contains(&key.as_str()) {
                self.unknown.insert(key, value);
            }
        }
    }
}

/// Why saving failed.
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    /// JSON serialization failed.
    #[error("cannot serialize settings: {0}")]
    Serialize(serde_json::Error),
    /// Filesystem write failed.
    #[error("cannot write settings: {0}")]
    Io(std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip() {
        let settings = Settings::default();
        let json = settings.to_json().expect("serialize");
        let parsed: Settings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.theme, ThemeMode::System);
        assert_eq!(parsed.schema, CURRENT_SCHEMA);
    }

    #[test]
    fn unknown_keys_are_preserved() {
        let json = r#"{"schema":1,"theme":"dark","futureThing":{"a":1}}"#;
        let mut settings: Settings = serde_json::from_str(json).expect("parse");
        settings.capture_unknown(json);
        let out = settings.to_json().expect("serialize");
        assert!(out.contains("futureThing"));
        assert!(out.contains("\"a\": 1"));
    }

    #[test]
    fn missing_fields_use_defaults() {
        let json = r#"{"theme":"light"}"#;
        let settings: Settings = serde_json::from_str(json).expect("parse");
        assert_eq!(settings.theme, ThemeMode::Light);
        assert_eq!(settings.schema, CURRENT_SCHEMA);
        assert!(settings.include_hidden);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = std::env::temp_dir().join("ferriscan-settings-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let path = dir.join("settings.json");
        let settings = Settings {
            theme: ThemeMode::Dark,
            ..Settings::default()
        };
        settings.save(&path).expect("save");
        let loaded = Settings::load(&path);
        assert_eq!(loaded.theme, ThemeMode::Dark);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join("ferriscan-settings-corrupt");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let path = dir.join("settings.json");
        std::fs::write(&path, "{not json").expect("write");
        let loaded = Settings::load(&path);
        assert_eq!(loaded.theme, ThemeMode::System);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
