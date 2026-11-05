//! Shared application state.

use std::sync::{Arc, Mutex, PoisonError};

use ferriscan_core::model::Snapshot;
use ferriscan_core::scan::controller::CancelToken;
use ferriscan_core::scan::progress::Progress;
use ferriscan_persist::{AppDirs, Settings, app_dirs};

/// State shared by every command.
///
/// `std::sync::RwLock` is used rather than a third-party lock: the write paths
/// are once-per-scan and once-per-settings-change, so contention is nil and the
/// dependency surface stays small.
#[derive(Debug)]
pub struct AppState {
    /// Latest finished snapshot, replaced atomically on each scan.
    snapshot: Mutex<Option<Arc<Snapshot>>>,
    /// Counters and state of the current or last scan.
    pub progress: Arc<Progress>,
    /// Cancellation token for the current scan.
    pub cancel: Arc<CancelToken>,
    /// Guards against starting a second scan.
    scanning: Mutex<bool>,
    /// User settings, loaded once at startup.
    settings: Mutex<Settings>,
    /// Resolved application directories.
    pub dirs: AppDirs,
    /// Monotonic snapshot counter.
    counter: Mutex<u64>,
}

impl AppState {
    /// Builds the state, loading settings from disk.
    #[must_use]
    pub fn new() -> Self {
        let dirs = app_dirs("FerriScan");
        let settings = Settings::load(&dirs.settings_file());
        Self {
            snapshot: Mutex::new(None),
            progress: Arc::new(Progress::new()),
            cancel: Arc::new(CancelToken::new()),
            scanning: Mutex::new(false),
            settings: Mutex::new(settings),
            dirs,
            counter: Mutex::new(0),
        }
    }

    /// Stores a snapshot and returns its display id.
    pub fn publish(&self, mut snapshot: Snapshot) -> u64 {
        let mut counter = lock(&self.counter);
        *counter += 1;
        snapshot.id = ferriscan_core::model::SnapshotId(*counter);
        let id = *counter;
        drop(counter);
        *lock(&self.snapshot) = Some(Arc::new(snapshot));
        id
    }

    /// Current snapshot, if any.
    #[must_use]
    pub fn current(&self) -> Option<Arc<Snapshot>> {
        lock(&self.snapshot).clone()
    }

    /// Whether a scan is running.
    pub fn is_scanning(&self) -> bool {
        *lock(&self.scanning)
    }

    /// Marks a scan as started; returns `false` when one was already running.
    pub fn begin_scan(&self) -> bool {
        let mut scanning = lock(&self.scanning);
        if *scanning {
            return false;
        }
        *scanning = true;
        true
    }

    /// Marks the scan as finished.
    pub fn end_scan(&self) {
        *lock(&self.scanning) = false;
    }

    /// Copy of the current settings.
    #[must_use]
    pub fn settings(&self) -> Settings {
        lock(&self.settings).clone()
    }

    /// Replaces the settings and persists them.
    pub fn replace_settings(&self, next: Settings) -> Result<(), String> {
        next.save(&self.dirs.settings_file())
            .map_err(|e| e.to_string())?;
        *lock(&self.settings) = next;
        Ok(())
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
