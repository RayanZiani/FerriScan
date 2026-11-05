//! Raw directory entries and the [`DirReader`] contract.

use std::path::Path;

use crate::scan::errors::ErrorCode;

/// Kind of a filesystem entry as reported by the OS.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntryKind {
    /// Regular file.
    File,
    /// Directory.
    Dir,
    /// Symbolic link or reparse point.
    Symlink,
    /// Anything else (device, FIFO, socket); recorded but never opened.
    Other,
}

/// One entry as returned by a [`DirReader`], before store insertion.
///
/// `name` is only the file name, never a full path: paths are reconstructed on
/// demand from the parent chain, which keeps the index small.
#[derive(Clone, Debug)]
pub struct RawEntry {
    /// Final path component, as valid UTF-8 (lossy on Unix).
    pub name: Box<str>,
    /// Entry kind.
    pub kind: EntryKind,
    /// Logical size in bytes.
    pub size: u64,
    /// On-disk size in bytes; equals `size` when the platform does not report it.
    pub allocated: u64,
    /// Modification time, Unix seconds; `0` when unknown.
    pub mtime: i64,
    /// Whether the OS marks the entry hidden.
    pub hidden: bool,
    /// Whether the OS marks the entry read-only.
    pub readonly: bool,
    /// Whether the OS marks the entry as system.
    pub system: bool,
    /// Whether the entry is a hard link already counted elsewhere.
    pub shared: bool,
}

impl RawEntry {
    /// Convenience constructor for tests and fakes.
    #[must_use]
    pub fn file(name: impl Into<Box<str>>, size: u64) -> Self {
        Self {
            name: name.into(),
            kind: EntryKind::File,
            size,
            allocated: size,
            mtime: 0,
            hidden: false,
            readonly: false,
            system: false,
            shared: false,
        }
    }

    /// Convenience constructor for a directory.
    #[must_use]
    pub fn dir(name: impl Into<Box<str>>) -> Self {
        Self {
            name: name.into(),
            kind: EntryKind::Dir,
            size: 0,
            allocated: 0,
            mtime: 0,
            hidden: false,
            readonly: false,
            system: false,
            shared: false,
        }
    }

    /// Convenience constructor for a symbolic link.
    #[must_use]
    pub fn symlink(name: impl Into<Box<str>>) -> Self {
        Self {
            name: name.into(),
            kind: EntryKind::Symlink,
            size: 0,
            allocated: 0,
            mtime: 0,
            hidden: false,
            readonly: false,
            system: false,
            shared: false,
        }
    }
}

/// Failure while reading a directory or one of its entries.
#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct FsError {
    /// Stable code used by the UI and the issues panel.
    pub code: ErrorCode,
    /// Raw OS error number, when there is one.
    pub os_code: Option<i32>,
    /// Human-readable detail; never localized, never shown raw to the user.
    pub message: String,
}

impl FsError {
    /// Builds an error from a code and a message.
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            os_code: None,
            message: message.into(),
        }
    }

    /// Attaches the raw OS error number.
    #[must_use]
    pub fn with_os(mut self, os_code: i32) -> Self {
        self.os_code = Some(os_code);
        self
    }

    /// Maps a `std::io::Error` to the engine taxonomy.
    #[must_use]
    pub fn from_io(err: &std::io::Error, path: &Path) -> Self {
        let code = match err.kind() {
            std::io::ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
            std::io::ErrorKind::NotFound => ErrorCode::NotFound,
            std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => {
                ErrorCode::Invalid
            }
            _ => ErrorCode::Io,
        };
        Self {
            code,
            os_code: err.raw_os_error(),
            message: format!("{} ({})", err, path.display()),
        }
    }
}

/// Lists the entries of one directory.
///
/// Implementations must not recurse: the engine owns traversal, depth limits and
/// loop detection. `out` is cleared and reused to keep the scan allocation-free
/// at steady state.
pub trait DirReader: Send + Sync {
    /// Reads `dir` and appends its direct entries to `out`.
    fn read_dir(&self, dir: &Path, out: &mut Vec<RawEntry>) -> Result<(), FsError>;

    /// Stable identifier for the filesystem holding `path`, used to stop a scan
    /// at a mount boundary. Returns `None` when unknown.
    fn device_id(&self, _path: &Path) -> Option<u64> {
        None
    }
}
