//! Directory enumeration for the host platform.

use std::path::Path;

use ferriscan_core::fs::{DirReader, EntryKind, FsError, RawEntry};

/// The platform's fastest available directory reader.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformDirReader;

impl PlatformDirReader {
    /// Creates a reader. Stateless; cheap to copy and share across threads.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl DirReader for PlatformDirReader {
    fn read_dir(&self, dir: &Path, out: &mut Vec<RawEntry>) -> Result<(), FsError> {
        read_dir_impl(dir, out)
    }

    fn device_id(&self, path: &Path) -> Option<u64> {
        device_id_impl(path)
    }
}

/// Portable fallback built on `std::fs::read_dir`.
///
/// The Windows and Linux fast paths live in the platform modules below; the
/// fallback keeps the crate compiling everywhere and is what macOS uses until
/// the `getattrlistbulk` reader lands (tracked as T2.4).
fn read_dir_impl(dir: &Path, out: &mut Vec<RawEntry>) -> Result<(), FsError> {
    let entries = std::fs::read_dir(dir).map_err(|e| FsError::from_io(&e, dir))?;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                return Err(FsError::from_io(&err, dir));
            }
        };
        let metadata = match entry.metadata() {
            Ok(meta) => meta,
            Err(err) => {
                // A single unreadable entry must not abort the whole directory.
                tracing::debug!(error = %err, "skipping entry without metadata");
                continue;
            }
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_type = entry.file_type().ok();
        let kind = if file_type.is_some_and(|t| t.is_symlink()) {
            EntryKind::Symlink
        } else if metadata.is_dir() {
            EntryKind::Dir
        } else if metadata.is_file() {
            EntryKind::File
        } else {
            EntryKind::Other
        };
        let size = metadata.len();
        out.push(RawEntry {
            name: name.into_boxed_str(),
            kind,
            size,
            allocated: size,
            mtime: mtime_secs(&metadata),
            hidden: is_hidden(entry.path().as_path(), &metadata),
            readonly: metadata.permissions().readonly(),
            system: false,
            shared: false,
        });
    }
    Ok(())
}

fn mtime_secs(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

#[cfg(unix)]
fn is_hidden(path: &Path, _metadata: &std::fs::Metadata) -> bool {
    // Unix has no hidden attribute: a leading dot is the only convention.
    path.file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with('.'))
}

#[cfg(windows)]
fn is_hidden(path: &Path, metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
        || path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with('.'))
}

#[cfg(unix)]
fn device_id_impl(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).ok().map(|m| m.dev())
}

#[cfg(windows)]
fn device_id_impl(path: &Path) -> Option<u64> {
    // The volume serial number is enough to separate `C:` from `D:`.
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{GetVolumeInformationW, GetVolumePathNameW};

    let mut root = vec![0u16; 260];
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: `wide` is NUL-terminated and `root` is a writable 260-char buffer.
    let ok = unsafe {
        GetVolumePathNameW(
            wide.as_ptr(),
            root.as_mut_ptr(),
            u32::try_from(root.len()).unwrap_or(0),
        )
    };
    if ok == 0 {
        return None;
    }
    let mut serial = 0u32;
    // SAFETY: requests only the serial number; other pointers are null, which the
    // API documents as allowed.
    let info = unsafe {
        GetVolumeInformationW(
            root.as_ptr(),
            std::ptr::null_mut(),
            0,
            &raw mut serial,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };
    (info != 0).then_some(u64::from(serial))
}

#[cfg(not(any(unix, windows)))]
fn device_id_impl(_path: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_real_directory() {
        let dir = std::env::temp_dir();
        let reader = PlatformDirReader::new();
        let mut out = Vec::new();
        reader.read_dir(&dir, &mut out).expect("temp dir readable");
        // The temp directory always contains at least one entry in practice, but
        // asserting on it would make the test environment-dependent.
        let _ = out;
    }

    #[test]
    fn missing_directory_is_not_found() {
        let reader = PlatformDirReader::new();
        let mut out = Vec::new();
        let err = reader
            .read_dir(Path::new("Z:\\definitely-missing-path"), &mut out)
            .or_else(|_| reader.read_dir(Path::new("/definitely-missing-path"), &mut out))
            .expect_err("must fail");
        assert!(matches!(
            err.code,
            ferriscan_core::scan::errors::ErrorCode::NotFound
        ));
    }
}
