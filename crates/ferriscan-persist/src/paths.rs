//! Per-platform application directories.
//!
//! Paths follow each OS convention and are resolved without extra crates so the
//! dependency surface stays small and auditable.

use std::path::PathBuf;

/// Directories used by the application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppDirs {
    /// Configuration (settings.json).
    pub config: PathBuf,
    /// Application data (journals).
    pub data: PathBuf,
    /// Cache (optional snapshot cache).
    pub cache: PathBuf,
    /// Logs.
    pub log: PathBuf,
}

impl AppDirs {
    /// Path of the settings file.
    #[must_use]
    pub fn settings_file(&self) -> PathBuf {
        self.config.join("settings.json")
    }

    /// Path of the journal directory.
    #[must_use]
    pub fn journal_dir(&self) -> PathBuf {
        self.data.join("journal")
    }

    /// Creates every directory, ignoring "already exists" errors.
    pub fn ensure(&self) -> std::io::Result<()> {
        for dir in [&self.config, &self.data, &self.cache, &self.log] {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::create_dir_all(self.journal_dir())?;
        Ok(())
    }
}

/// Resolves the application directories for the current user.
///
/// `app_name` is inserted as a directory component so a future portable mode can
/// point at a different root without changing the call sites.
#[must_use]
pub fn app_dirs(app_name: &str) -> AppDirs {
    let home = home_dir();
    #[cfg(windows)]
    {
        let base = std::env::var_os("APPDATA")
            .map_or_else(|| home.join("AppData").join("Roaming"), PathBuf::from);
        let local = std::env::var_os("LOCALAPPDATA")
            .map_or_else(|| home.join("AppData").join("Local"), PathBuf::from);
        AppDirs {
            config: base.join(app_name),
            data: base.join(app_name),
            cache: local.join(app_name).join("cache"),
            log: local.join(app_name).join("logs"),
        }
    }
    #[cfg(target_os = "macos")]
    {
        let support = home.join("Library").join("Application Support");
        let caches = home.join("Library").join("Caches");
        AppDirs {
            config: support.join(app_name),
            data: support.join(app_name),
            cache: caches.join(app_name),
            log: home.join("Library").join("Logs").join(app_name),
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let config_home = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local").join("share"));
        let cache_home = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".cache"));
        let name = app_name.to_lowercase();
        AppDirs {
            config: config_home.join(&name),
            data: data_home.join(&name),
            cache: cache_home.join(&name),
            log: data_home.join(&name).join("logs"),
        }
    }
    #[cfg(not(any(windows, unix)))]
    {
        let base = home.join(app_name);
        AppDirs {
            config: base.clone(),
            data: base.clone(),
            cache: base.join("cache"),
            log: base.join("logs"),
        }
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_file_lives_in_config() {
        let dirs = app_dirs("FerriScan");
        assert!(dirs.settings_file().ends_with("settings.json"));
        assert!(dirs.journal_dir().ends_with("journal"));
    }

    #[test]
    fn dirs_are_absolute_or_explicit() {
        let dirs = app_dirs("FerriScan");
        assert!(!dirs.config.as_os_str().is_empty());
    }
}
