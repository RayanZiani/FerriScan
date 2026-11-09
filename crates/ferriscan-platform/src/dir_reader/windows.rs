//! Windows bulk directory reader.
//!
//! `std::fs::read_dir` issues one syscall per entry. Here one `GetFileInformationByHandleEx`
//! call returns as many `FILE_FULL_DIR_INFO` records as fit in a 64 KiB buffer, carrying size,
//! allocation size, attributes and mtime, so no per-file `stat` is needed. No elevation.

use std::cell::RefCell;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use ferriscan_core::fs::{EntryKind, FsError, RawEntry};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_READONLY,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_SYSTEM, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FULL_DIR_INFO, FILE_LIST_DIRECTORY, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    FileFullDirectoryInfo, FileFullDirectoryRestartInfo, GetFileInformationByHandleEx,
    OPEN_EXISTING,
};

/// Buffer size in `u64` words (64 KiB); `u64` guarantees the 8-byte alignment the API requires.
const BUF_WORDS: usize = 8 * 1024;
/// Reparse tags that must not be followed (symlink, junction).
const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xA000_0003;
const IO_REPARSE_TAG_SYMLINK: u32 = 0xA000_000C;
/// Seconds between 1601-01-01 (FILETIME epoch) and 1970-01-01.
const FILETIME_UNIX_DELTA: i64 = 11_644_473_600;

thread_local! {
    static BUFFER: RefCell<Vec<u64>> = RefCell::new(vec![0u64; BUF_WORDS]);
}

struct Handle(windows_sys::Win32::Foundation::HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle was returned by a successful `CreateFileW` and is closed once.
        unsafe { CloseHandle(self.0) };
    }
}

pub(super) fn read_dir(dir: &Path, out: &mut Vec<RawEntry>) -> Result<(), FsError> {
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: `wide` is NUL-terminated; the null security/template arguments are documented
    // as allowed. The returned handle is wrapped in `Handle` so it is always closed.
    let raw = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_LIST_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(FsError::from_io(&std::io::Error::last_os_error(), dir));
    }
    let handle = Handle(raw);

    BUFFER.with(|cell| {
        let mut buf = cell.borrow_mut();
        let byte_len = u32::try_from(buf.len() * 8).unwrap_or(u32::MAX);
        let mut class = FileFullDirectoryRestartInfo;
        loop {
            // SAFETY: `buf` is a live, 8-byte-aligned allocation of `byte_len` bytes and the
            // handle is a valid directory handle opened with `FILE_LIST_DIRECTORY`.
            let ok = unsafe {
                GetFileInformationByHandleEx(handle.0, class, buf.as_mut_ptr().cast(), byte_len)
            };
            class = FileFullDirectoryInfo;
            if ok == 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
                    return Ok(());
                }
                return Err(FsError::from_io(&err, dir));
            }
            parse(buf.as_ptr().cast(), out);
        }
    })
}

/// Walks the packed `FILE_FULL_DIR_INFO` records the kernel wrote into the buffer.
#[allow(clippy::cast_ptr_alignment)] // records are 8-byte aligned; see SAFETY below
fn parse(base: *const u8, out: &mut Vec<RawEntry>) {
    let mut offset = 0usize;
    loop {
        // SAFETY: the kernel guarantees each record starts at an 8-byte aligned offset inside
        // the buffer and that `NextEntryOffset` chains to the next record or is zero.
        let info = unsafe { &*base.add(offset).cast::<FILE_FULL_DIR_INFO>() };
        // SAFETY: `FileName` is a variable-length array of `FileNameLength` bytes that the
        // kernel stored inside the same record.
        let name = unsafe {
            std::slice::from_raw_parts(info.FileName.as_ptr(), info.FileNameLength as usize / 2)
        };
        if name != [u16::from(b'.')] && name != [u16::from(b'.'), u16::from(b'.')] {
            out.push(entry(info, name));
        }
        if info.NextEntryOffset == 0 {
            return;
        }
        offset += info.NextEntryOffset as usize;
    }
}

fn entry(info: &FILE_FULL_DIR_INFO, name: &[u16]) -> RawEntry {
    let attrs = info.FileAttributes;
    let name = String::from_utf16_lossy(name);
    // For reparse points `EaSize` holds the reparse tag (documented for this structure).
    let is_link = attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0
        && matches!(
            info.EaSize,
            IO_REPARSE_TAG_SYMLINK | IO_REPARSE_TAG_MOUNT_POINT
        );
    let kind = if is_link {
        EntryKind::Symlink
    } else if attrs & FILE_ATTRIBUTE_DIRECTORY != 0 {
        EntryKind::Dir
    } else {
        EntryKind::File
    };
    let size = u64::try_from(info.EndOfFile).unwrap_or(0);
    RawEntry {
        hidden: attrs & FILE_ATTRIBUTE_HIDDEN != 0 || name.starts_with('.'),
        readonly: attrs & FILE_ATTRIBUTE_READONLY != 0,
        system: attrs & FILE_ATTRIBUTE_SYSTEM != 0,
        size,
        allocated: u64::try_from(info.AllocationSize).unwrap_or(size),
        mtime: info.LastWriteTime / 10_000_000 - FILETIME_UNIX_DELTA,
        name: name.into_boxed_str(),
        kind,
        shared: false,
        key: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_std_for_sizes_and_kinds() {
        let dir = std::env::temp_dir().join(format!("ferriscan-win-reader-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.bin"), vec![0u8; 1234]).unwrap();
        std::fs::write(dir.join(".dot"), b"x").unwrap();

        let mut out = Vec::new();
        read_dir(&dir, &mut out).unwrap();
        out.sort_by(|a, b| a.name.cmp(&b.name));

        let names: Vec<&str> = out.iter().map(|e| &*e.name).collect();
        assert_eq!(names, [".dot", "a.bin", "sub"]);
        assert!(out[0].hidden);
        assert_eq!((out[1].kind, out[1].size), (EntryKind::File, 1234));
        assert!(out[1].allocated >= 1234);
        assert_eq!(out[2].kind, EntryKind::Dir);
        assert!(out[1].mtime > 1_600_000_000);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
