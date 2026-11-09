//! Raw volume access for the MFT reader. Needs an elevated process (ADR-0006).

use std::fs::{File, OpenOptions};
use std::os::windows::fs::{FileExt, OpenOptionsExt};
use std::os::windows::io::AsRawHandle;
use std::path::{Component, Path, Prefix};
use std::sync::atomic::{AtomicUsize, Ordering};

use ferriscan_core::fs::FsError;
use ferriscan_core::scan::errors::ErrorCode;
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Ioctl::{FSCTL_GET_NTFS_VOLUME_DATA, NTFS_VOLUME_DATA_BUFFER};

use super::record::{self, Record};
use super::{IndexBuilder, MftIndex};

/// Bytes read per request: big enough for sequential throughput, small enough to spread over
/// worker threads. A multiple of every record and sector size NTFS uses.
const CHUNK: u64 = 4 << 20;
/// Share read, write and delete so opening the live volume never blocks other programs.
const SHARE_ALL: u32 = 7;

fn io(err: &std::io::Error, what: &str) -> FsError {
    FsError::from_io(err, Path::new(what))
}

/// `\\.\C:` for a path on drive C.
fn volume_path(root: &Path) -> Result<String, FsError> {
    match root.components().next() {
        Some(Component::Prefix(p)) => match p.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                Ok(format!("\\\\.\\{}:", char::from(letter)))
            }
            _ => Err(FsError::new(ErrorCode::Unsupported, "not a local drive")),
        },
        _ => Err(FsError::new(ErrorCode::Unsupported, "not a drive path")),
    }
}

fn volume_data(file: &File) -> Result<NTFS_VOLUME_DATA_BUFFER, FsError> {
    // SAFETY: an all-zero NTFS_VOLUME_DATA_BUFFER is a valid plain-data value.
    let mut data: NTFS_VOLUME_DATA_BUFFER = unsafe { std::mem::zeroed() };
    let mut returned = 0u32;
    let size = u32::try_from(std::mem::size_of::<NTFS_VOLUME_DATA_BUFFER>()).unwrap_or(u32::MAX);
    // SAFETY: the handle is a live volume handle owned by `file`; the output buffer is a valid
    // writable `NTFS_VOLUME_DATA_BUFFER` of `size` bytes; no input buffer or overlapped I/O.
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            FSCTL_GET_NTFS_VOLUME_DATA,
            std::ptr::null(),
            0,
            std::ptr::addr_of_mut!(data).cast(),
            size,
            &raw mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        // Typically ERROR_INVALID_FUNCTION on a non-NTFS volume.
        return Err(FsError::new(ErrorCode::Unsupported, "not an NTFS volume"));
    }
    Ok(data)
}

/// One contiguous piece of the MFT: where it sits in the table and on the volume.
#[derive(Clone, Copy)]
struct Chunk {
    mft_offset: u64,
    volume_offset: u64,
    len: u64,
}

fn plan_chunks(runs: &[(u64, u64)], cluster: u64, valid_len: u64, sector: u64) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut mft_offset = 0u64;
    for &(lcn, count) in runs {
        let run_bytes = count * cluster;
        let mut done = 0;
        while done < run_bytes && mft_offset < valid_len {
            let want = (valid_len - mft_offset).min(run_bytes - done).min(CHUNK);
            // Volume reads must be sector aligned; runs are cluster aligned so this stays inside.
            let len = want.div_ceil(sector) * sector;
            chunks.push(Chunk {
                mft_offset,
                volume_offset: lcn * cluster + done,
                len: len.min(run_bytes - done),
            });
            mft_offset += want;
            done += want;
        }
    }
    chunks
}

fn read_exact_at(file: &File, buf: &mut [u8], mut offset: u64) -> Result<(), FsError> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = file
            .seek_read(&mut buf[filled..], offset)
            .map_err(|e| io(&e, "volume"))?;
        if n == 0 {
            return Err(FsError::new(ErrorCode::Io, "unexpected end of volume"));
        }
        filled += n;
        offset += n as u64;
    }
    Ok(())
}

/// Reads and indexes the whole MFT of the volume holding `root`.
pub(super) fn read_index(root: &Path) -> Result<MftIndex, FsError> {
    let path = volume_path(root)?;
    let file = OpenOptions::new()
        .read(true)
        .share_mode(SHARE_ALL)
        .open(&path)
        .map_err(|e| io(&e, &path))?;
    let vol = volume_data(&file)?;

    let rec_size = u64::from(vol.BytesPerFileRecordSegment);
    let cluster = u64::from(vol.BytesPerCluster);
    let sector = u64::from(vol.BytesPerSector).max(512);
    let bad = |m: &str| FsError::new(ErrorCode::Invalid, m);
    let (Ok(mft_lcn), Ok(valid_len)) = (
        u64::try_from(vol.MftStartLcn),
        u64::try_from(vol.MftValidDataLength),
    ) else {
        return Err(bad("negative MFT geometry"));
    };
    if !(512..=65_536).contains(&rec_size) || cluster == 0 || rec_size % 512 != 0 {
        return Err(bad("unexpected MFT geometry"));
    }
    let rec_len = rec_size as usize;

    // Record 0 describes the MFT itself, including where its data lives.
    let mut first = vec![0u8; rec_size.max(sector) as usize];
    read_exact_at(&file, &mut first, mft_lcn * cluster)?;
    let runs = record::mft_runs(&mut first[..rec_len]).ok_or_else(|| bad("bad $MFT record"))?;
    let chunks = plan_chunks(&runs, cluster, valid_len, sector);

    let threads = std::thread::available_parallelism()
        .map_or(4, std::num::NonZeroUsize::get)
        .clamp(2, 8);
    let next = AtomicUsize::new(0);
    let results: Vec<Result<Vec<(u64, Record)>, FsError>> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| -> Result<Vec<(u64, Record)>, FsError> {
                    let mut found = Vec::new();
                    let mut buf = vec![0u8; CHUNK as usize];
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(chunk) = chunks.get(i) else {
                            return Ok(found);
                        };
                        let len = chunk.len as usize;
                        read_exact_at(&file, &mut buf[..len], chunk.volume_offset)?;
                        // Only whole records inside the valid MFT length.
                        let used = (valid_len - chunk.mft_offset).min(chunk.len) as usize;
                        for (n, rec) in buf[..used].chunks_exact_mut(rec_len).enumerate() {
                            let number = chunk.mft_offset / rec_size + n as u64;
                            if let Some(parsed) = record::parse(rec, number) {
                                found.push((number, parsed));
                            }
                        }
                    }
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|w| {
                w.join()
                    .unwrap_or_else(|_| Err(FsError::new(ErrorCode::Io, "MFT worker panicked")))
            })
            .collect()
    });

    let mut builder = IndexBuilder::default();
    for part in results {
        for (number, rec) in part? {
            builder.add(number, &rec);
        }
    }
    Ok(builder.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_cover_the_valid_length_across_runs() {
        // Two runs of 8 and 4 clusters of 4 KiB (48 KiB); only 40 KiB are valid.
        let chunks = plan_chunks(&[(100, 8), (500, 4)], 4096, 40 * 1024, 512);
        let covered: u64 = chunks.iter().map(|c| c.len).sum();
        assert_eq!(covered, 40 * 1024);
        assert_eq!(chunks[0].volume_offset, 100 * 4096);
        assert_eq!(chunks[1].mft_offset, 32 * 1024);
        assert_eq!(chunks[1].volume_offset, 500 * 4096);
    }

    #[test]
    fn volume_path_needs_a_drive() {
        assert_eq!(volume_path(Path::new("C:\\")).unwrap(), "\\\\.\\C:");
        assert!(volume_path(Path::new("\\\\server\\share")).is_err());
    }
}
