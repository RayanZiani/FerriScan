//! Local persistence: settings, paths and (later) the action journal.
//!
//! Nothing here ever contacts the network. Settings are a versioned JSON file
//! written atomically; unknown keys are preserved so a newer build's file
//! survives a downgrade without losing the user's choices.

#![forbid(unsafe_code)]

pub mod paths;
pub mod settings;

pub use paths::{AppDirs, app_dirs};
pub use settings::{Settings, ThemeMode, Units};
