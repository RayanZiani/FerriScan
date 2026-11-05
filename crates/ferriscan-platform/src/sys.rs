//! Platform name used in diagnostics and the About dialog.

/// Canonical OS name: `windows`, `linux`, `macos`, or `unknown`.
#[must_use]
pub const fn platform_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    }
}
