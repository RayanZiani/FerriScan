//! Volume discovery, normalized across platforms.

use std::path::PathBuf;

/// One mounted volume.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    /// Mount point, e.g. `C:\` or `/`.
    pub mount_point: PathBuf,
    /// Display label, falling back to the last path component.
    pub label: String,
    /// Filesystem name, e.g. `NTFS`, `ext4`.
    pub filesystem: String,
    /// Kind of volume.
    pub kind: VolumeKind,
    /// Capacity in bytes.
    pub total_bytes: u64,
    /// Free bytes available to the current user.
    pub available_bytes: u64,
    /// Read-only flag.
    pub read_only: bool,
    /// Locked volumes (`BitLocker`) cannot be read; they are listed but not scannable.
    pub locked: bool,
}

/// Volume category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeKind {
    /// Internal fixed disk.
    Fixed,
    /// Removable media.
    Removable,
    /// Network share.
    Network,
    /// Optical media.
    Optical,
    /// RAM disk.
    Ram,
    /// Unknown or unsupported.
    Unknown,
}

impl VolumeKind {
    /// Stable lowercase name used in DTOs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::Removable => "removable",
            Self::Network => "network",
            Self::Optical => "optical",
            Self::Ram => "ram",
            Self::Unknown => "unknown",
        }
    }
}

/// Lists every mounted volume the user can browse.
///
/// Errors are swallowed per volume: a broken mount must not hide the rest of
/// the machine.
#[must_use]
pub fn list_volumes() -> Vec<Volume> {
    sys_volumes()
}

#[cfg(windows)]
fn sys_volumes() -> Vec<Volume> {
    windows_impl::list()
}

#[cfg(target_os = "linux")]
fn sys_volumes() -> Vec<Volume> {
    unix_impl::list_linux()
}

#[cfg(target_os = "macos")]
fn sys_volumes() -> Vec<Volume> {
    unix_impl::list_macos()
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn sys_volumes() -> Vec<Volume> {
    Vec::new()
}

#[cfg(windows)]
mod windows_impl {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;

    use windows_sys::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDriveStringsW, GetVolumeInformationW,
    };

    use super::{Volume, VolumeKind};

    const MAX_PATH: usize = 260;

    pub fn list() -> Vec<Volume> {
        let mut buffer = vec![0u16; 4 * MAX_PATH];
        // SAFETY: the buffer is sized for the documented maximum of
        // GetLogicalDriveStringsW (4 * MAX_PATH wide chars) and we pass its
        // exact length; the call only writes inside that range.
        let len = unsafe {
            GetLogicalDriveStringsW(
                u32::try_from(buffer.len()).unwrap_or(0),
                buffer.as_mut_ptr(),
            )
        };
        if len == 0 {
            return Vec::new();
        }

        buffer.truncate(len as usize);
        let mut out = Vec::new();
        for raw in buffer.split(|c| *c == 0).filter(|s| !s.is_empty()) {
            let mut wide: Vec<u16> = raw.to_vec();
            wide.push(0);
            let path = PathBuf::from(OsString::from_wide(raw));
            out.push(describe(&wide, path));
        }
        out.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
        out
    }

    fn describe(wide: &[u16], mount_point: PathBuf) -> Volume {
        // SAFETY: `wide` is NUL-terminated and valid for the duration of the call.
        let drive_type = unsafe { GetDriveTypeW(wide.as_ptr()) };
        let kind = match drive_type {
            2 => VolumeKind::Removable,
            3 => VolumeKind::Fixed,
            4 => VolumeKind::Network,
            5 => VolumeKind::Optical,
            6 => VolumeKind::Ram,
            _ => VolumeKind::Unknown,
        };

        let mut fs_name = vec![0u16; MAX_PATH];
        let mut volume_name = vec![0u16; MAX_PATH];
        // SAFETY: all buffers are MAX_PATH wide chars and the pointers are valid.
        let info_ok = unsafe {
            GetVolumeInformationW(
                wide.as_ptr(),
                volume_name.as_mut_ptr(),
                u32::try_from(volume_name.len()).unwrap_or(0),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                fs_name.as_mut_ptr(),
                u32::try_from(fs_name.len()).unwrap_or(0),
            )
        };

        let locked = info_ok == 0;
        let label = if info_ok != 0 {
            let name = to_os_string(&volume_name);
            if name.is_empty() {
                mount_point.display().to_string()
            } else {
                name
            }
        } else {
            mount_point.display().to_string()
        };
        let filesystem = if info_ok != 0 {
            to_os_string(&fs_name)
        } else {
            String::new()
        };

        let mut free = 0u64;
        let mut total = 0u64;
        // SAFETY: `wide` is NUL-terminated; the output pointers are valid.
        let space_ok = unsafe {
            GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &raw mut free,
                &raw mut total,
                std::ptr::null_mut(),
            )
        };
        if space_ok == 0 && total == 0 {
            // Unformatted or locked volume: still list it so the UI can explain.
            return Volume {
                mount_point,
                label,
                filesystem,
                kind,
                total_bytes: 0,
                available_bytes: 0,
                read_only: true,
                locked: true,
            };
        }

        Volume {
            mount_point,
            label,
            filesystem,
            kind,
            total_bytes: total,
            available_bytes: free,
            read_only: false,
            locked,
        }
    }

    fn to_os_string(buffer: &[u16]) -> String {
        let end = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
        OsString::from_wide(&buffer[..end])
            .to_string_lossy()
            .into_owned()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_impl {
    use std::ffi::CStr;
    use std::path::PathBuf;

    use super::{Volume, VolumeKind};

    #[cfg(target_os = "linux")]
    pub fn list_linux() -> Vec<Volume> {
        // `/proc/self/mountinfo` is the authoritative, namespace-aware source.
        let Ok(raw) = std::fs::read_to_string("/proc/self/mountinfo") else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for line in raw.lines() {
            let mut fields = line.split(" - ");
            let Some(left) = fields.next() else { continue };
            let Some(right) = fields.next() else { continue };
            let left_parts: Vec<&str> = left.split_whitespace().collect();
            let right_parts: Vec<&str> = right.split_whitespace().collect();
            if left_parts.len() < 5 || right_parts.is_empty() {
                continue;
            }
            let filesystem = right_parts[0];
            if is_pseudo(filesystem) {
                continue;
            }
            let mount_point = unescape(left_parts[4]);
            let path = PathBuf::from(&mount_point);
            let Some((total, available)) = statvfs(&path) else {
                continue;
            };
            let kind = if filesystem.starts_with("nfs") || filesystem.starts_with("cifs") {
                VolumeKind::Network
            } else if filesystem == "tmpfs" || filesystem == "ramfs" {
                VolumeKind::Ram
            } else if filesystem == "iso9660" || filesystem == "udf" {
                VolumeKind::Optical
            } else {
                VolumeKind::Fixed
            };
            out.push(Volume {
                label: path
                    .file_name()
                    .map_or_else(|| mount_point.clone(), |n| n.to_string_lossy().into_owned()),
                mount_point: path,
                filesystem: filesystem.to_owned(),
                kind,
                total_bytes: total,
                available_bytes: available,
                read_only: false,
                locked: false,
            });
        }
        out.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
        out
    }

    #[cfg(target_os = "linux")]
    fn is_pseudo(fs: &str) -> bool {
        matches!(
            fs,
            "proc"
                | "sysfs"
                | "devtmpfs"
                | "devpts"
                | "cgroup"
                | "cgroup2"
                | "securityfs"
                | "debugfs"
                | "tracefs"
                | "configfs"
                | "fusectl"
                | "mqueue"
                | "hugetlbfs"
                | "pstore"
                | "bpf"
                | "autofs"
                | "binfmt_misc"
        )
    }

    #[cfg(target_os = "linux")]
    fn unescape(input: &str) -> String {
        let bytes = input.as_bytes();
        let mut out = String::with_capacity(input.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 3 < bytes.len() {
                let hex = &input[i + 1..i + 3];
                if let Ok(value) = u8::from_str_radix(hex, 8) {
                    out.push(value as char);
                    i += 4;
                    continue;
                }
            }
            out.push(bytes[i] as char);
            i += 1;
        }
        out
    }

    #[cfg(target_os = "linux")]
    fn statvfs(path: &std::path::Path) -> Option<(u64, u64)> {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
        // SAFETY: `statvfs` is zero-initialized and `c_path` is a valid,
        // NUL-terminated C string for the duration of the call.
        let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
        // SAFETY: see above; the out-pointer is valid and properly aligned.
        let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stats) };
        if rc != 0 {
            return None;
        }
        let block = u64::from(stats.f_frsize.max(1));
        Some((
            stats.f_blocks.saturating_mul(block),
            stats.f_bavail.saturating_mul(block),
        ))
    }

    #[cfg(target_os = "macos")]
    pub fn list_macos() -> Vec<Volume> {
        use std::ffi::CStr;
        use std::mem::MaybeUninit;

        // SAFETY: `getmntinfo` writes into the provided pointer; we pass a valid
        // MaybeUninit and check the returned count before reading.
        let mut info: *mut libc::statfs = std::ptr::null_mut();
        // SAFETY: standard libc call; the pointer stays valid until the next call.
        let count = unsafe { libc::getmntinfo(&mut info, libc::MNT_NOWAIT) };
        if count <= 0 || info.is_null() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for i in 0..count as usize {
            // SAFETY: `getmntinfo` guarantees `count` contiguous entries.
            let entry = unsafe { &*info.add(i) };
            // SAFETY: the kernel guarantees NUL-terminated strings in statfs.
            let fs = unsafe { CStr::from_ptr(entry.f_fstypename.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            if fs == "devfs" || fs == "autofs" {
                continue;
            }
            // SAFETY: same guarantee for the mount point.
            let mount = unsafe { CStr::from_ptr(entry.f_mntonname.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            let path = PathBuf::from(&mount);
            let block = u64::from(entry.f_bsize.max(1));
            let total = entry.f_blocks.saturating_mul(block);
            let available = entry.f_bavail.saturating_mul(block);
            let read_only = entry.f_flags & u32::try_from(libc::MNT_RDONLY).unwrap_or(0) != 0;
            let kind = if fs == "nfs" || fs.starts_with("smbfs") {
                VolumeKind::Network
            } else if fs == "cd9660" || fs == "udf" {
                VolumeKind::Optical
            } else {
                VolumeKind::Fixed
            };
            out.push(Volume {
                label: path
                    .file_name()
                    .map_or_else(|| mount.clone(), |n| n.to_string_lossy().into_owned()),
                mount_point: path,
                filesystem: fs,
                kind,
                total_bytes: total,
                available_bytes: available,
                read_only,
                locked: false,
            });
        }
        out.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
        out
    }

    #[allow(dead_code)]
    fn unused(_: &CStr, _: MaybeUninit<libc::statfs>) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing_does_not_panic() {
        let volumes = list_volumes();
        for v in &volumes {
            assert!(!v.mount_point.as_os_str().is_empty());
            assert!(v.available_bytes <= v.total_bytes);
        }
    }
}
