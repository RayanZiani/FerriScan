//! Process elevation, for the opt-in MFT fast scan (ADR-0006).
//!
//! The app never elevates silently: [`relaunch_elevated`] is only called from an explicit user
//! action and shows the OS consent prompt. Only Windows is implemented.

use std::io;

/// Whether the current process runs with an elevated (administrator) token.
#[must_use]
pub fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        windows::is_elevated()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Starts a new elevated instance of this executable (UAC prompt). The caller should quit
/// afterwards so two windows do not stay open.
///
/// # Errors
///
/// Fails when the user declines the prompt or the OS cannot start the process. Returns
/// `Unsupported` off Windows.
pub fn relaunch_elevated() -> io::Result<()> {
    #[cfg(windows)]
    {
        windows::relaunch_elevated()
    }
    #[cfg(not(windows))]
    {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

#[cfg(windows)]
mod windows {
    use std::io;
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{
        GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows_sys::Win32::UI::Shell::ShellExecuteW;

    /// `SW_SHOWNORMAL`, spelled out to avoid pulling a whole windowing feature in.
    const SW_SHOWNORMAL: i32 = 1;

    pub(super) fn is_elevated() -> bool {
        let mut token: HANDLE = std::ptr::null_mut();
        // SAFETY: `GetCurrentProcess` returns a pseudo-handle that needs no closing; `token`
        // is a valid out-pointer.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) } == 0 {
            return false;
        }
        let mut info = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut returned = 0u32;
        let size = u32::try_from(std::mem::size_of::<TOKEN_ELEVATION>()).unwrap_or(0);
        // SAFETY: `token` is a live token handle opened with `TOKEN_QUERY`; `info` is a valid
        // writable `TOKEN_ELEVATION` of `size` bytes.
        let ok = unsafe {
            GetTokenInformation(
                token,
                TokenElevation,
                (&raw mut info).cast(),
                size,
                &raw mut returned,
            )
        };
        // SAFETY: `token` was opened above and is closed exactly once.
        unsafe { CloseHandle(token) };
        ok != 0 && info.TokenIsElevated != 0
    }

    pub(super) fn relaunch_elevated() -> io::Result<()> {
        let exe = std::env::current_exe()?;
        let file: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
        let verb: Vec<u16> = "runas\0".encode_utf16().collect();
        // SAFETY: both strings are NUL-terminated and outlive the call; null parameters,
        // working directory and window handle are documented as allowed.
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                file.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        // Values above 32 mean success (documented ShellExecute contract).
        if result as usize > 32 {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "elevation was declined or failed",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn elevation_query_does_not_panic() {
        // CI runners are often administrators, so only the call itself is checked.
        let _ = super::is_elevated();
    }
}
