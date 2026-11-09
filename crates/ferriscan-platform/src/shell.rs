//! Shell integration: recycle bin and "open with the default handler".
//!
//! No child process is spawned: both operations call OS shell APIs directly, so they need no
//! entry in the external-process allowlist. Only Windows is implemented; other platforms return
//! `ErrorKind::Unsupported` until their backends land (see `docs/PLATFORM_MATRIX.md`).

use std::io;
use std::path::Path;

/// Moves `path` to the recycle bin.
///
/// # Errors
///
/// Fails when the shell refuses the operation, when the user declines the OS warning shown for
/// items too large for the bin (which would otherwise be deleted permanently), or when the path
/// still exists afterwards. Returns `Unsupported` off Windows.
pub fn trash(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        windows::trash(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

/// Opens `path` with the OS default handler (a folder opens in the file manager).
///
/// # Errors
///
/// Fails when the shell cannot launch a handler. Returns `Unsupported` off Windows.
pub fn open_path(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        windows::open_path(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

#[cfg(windows)]
mod windows {
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows_sys::Win32::UI::Shell::{
        FO_DELETE, FOF_ALLOWUNDO, FOF_SILENT, FOF_WANTNUKEWARNING, SHFILEOPSTRUCTW,
        SHFileOperationW, ShellExecuteW,
    };

    /// `SW_SHOWNORMAL`, spelled out to avoid pulling a whole windowing feature in.
    const SW_SHOWNORMAL: i32 = 1;

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    pub(super) fn trash(path: &Path) -> io::Result<()> {
        // `pFrom` is a list of paths terminated by an extra NUL.
        let mut from = wide(path);
        from.push(0);
        let mut op = SHFILEOPSTRUCTW {
            hwnd: std::ptr::null_mut(),
            wFunc: FO_DELETE,
            pFrom: from.as_ptr(),
            pTo: std::ptr::null(),
            // Deliberately no FOF_NOCONFIRMATION: FOF_WANTNUKEWARNING then makes the shell ask
            // before permanently deleting an item that does not fit in the recycle bin.
            fFlags: u16::try_from(FOF_ALLOWUNDO | FOF_SILENT | FOF_WANTNUKEWARNING).unwrap_or(0),
            fAnyOperationsAborted: 0,
            hNameMappings: std::ptr::null_mut(),
            lpszProgressTitle: std::ptr::null(),
        };
        // SAFETY: `op` is fully initialized; `from` is double-NUL-terminated and outlives the
        // call; the null `hwnd`, `pTo` and title are documented as allowed.
        let code = unsafe { SHFileOperationW(&raw mut op) };
        if code != 0 {
            return Err(io::Error::other(format!(
                "shell delete failed (0x{code:x})"
            )));
        }
        if op.fAnyOperationsAborted != 0 {
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
        // Trust but verify: never report success for an item that is still there.
        if std::fs::symlink_metadata(path).is_ok() {
            return Err(io::Error::other("item still exists after delete"));
        }
        Ok(())
    }

    pub(super) fn open_path(path: &Path) -> io::Result<()> {
        let target = wide(path);
        let verb: Vec<u16> = "open\0".encode_utf16().collect();
        // SAFETY: both strings are NUL-terminated and outlive the call; null parameters, working
        // directory and window handle are documented as allowed.
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        // Values above 32 mean success (documented quirk of the legacy return type).
        if result as usize > 32 {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "open failed (code {})",
                result as usize
            )))
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn trashing_a_file_removes_it() {
        let file =
            std::env::temp_dir().join(format!("ferriscan-trash-test-{}.txt", std::process::id()));
        std::fs::write(&file, b"x").unwrap();
        trash(&file).unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn trashing_a_missing_file_is_an_error() {
        let file = std::env::temp_dir().join("ferriscan-definitely-missing-file.txt");
        assert!(trash(&file).is_err());
    }
}
