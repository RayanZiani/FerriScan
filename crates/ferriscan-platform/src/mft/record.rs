//! Pure parser for NTFS MFT file records.
//!
//! The bytes come from a raw volume and are untrusted: every offset is bounds-checked and a
//! malformed record yields `None` instead of an error, so one bad record never aborts a scan.
//! No OS calls here, so the parser is unit-tested on any platform.

/// `$STANDARD_INFORMATION`.
const ATTR_STANDARD_INFORMATION: u32 = 0x10;
/// `$FILE_NAME`.
const ATTR_FILE_NAME: u32 = 0x30;
/// `$DATA`.
const ATTR_DATA: u32 = 0x80;
/// `$REPARSE_POINT`.
const ATTR_REPARSE_POINT: u32 = 0xC0;
/// End-of-attributes marker.
const ATTR_END: u32 = 0xFFFF_FFFF;

/// Bytes covered by one update sequence entry.
const SECTOR: usize = 512;
/// Seconds between 1601-01-01 and 1970-01-01.
const FILETIME_UNIX_DELTA: i64 = 11_644_473_600;
/// Mask of the record number inside a 64-bit file reference.
const REF_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

/// File attribute bits as stored in `$STANDARD_INFORMATION`.
pub(crate) mod attr {
    pub const READONLY: u32 = 0x1;
    pub const HIDDEN: u32 = 0x2;
    pub const SYSTEM: u32 = 0x4;
    pub const REPARSE_POINT: u32 = 0x400;
}

/// Reparse tags that must not be followed (symlink, junction).
const TAG_SYMLINK: u32 = 0xA000_000C;
const TAG_MOUNT_POINT: u32 = 0xA000_0003;

/// What one in-use file record says about its file (or about its base file when it is an
/// extension record).
#[derive(Debug, Default, Clone)]
pub(crate) struct Record {
    /// Record number of the base file; equals the record's own number when it is a base record.
    pub base: u64,
    /// `true` when this is a base record (extension records only carry `$DATA` information).
    pub is_base: bool,
    pub is_dir: bool,
    /// Parent directory record number, from the preferred `$FILE_NAME`.
    pub parent: u64,
    pub name: Option<Box<str>>,
    /// Windows file attribute bits (`attr::*`).
    pub attrs: u32,
    /// Modification time, Unix seconds.
    pub mtime: i64,
    /// Logical size of the unnamed `$DATA` stream.
    pub size: u64,
    /// On-disk size of the unnamed `$DATA` stream.
    pub allocated: u64,
    /// Whether an unnamed `$DATA` header with VCN 0 was seen.
    pub has_data: bool,
    pub reparse_tag: u32,
}

impl Record {
    /// Whether the entry is a link the scan must not descend into.
    pub(crate) const fn is_link(&self) -> bool {
        self.attrs & attr::REPARSE_POINT != 0
            && (self.reparse_tag == TAG_SYMLINK || self.reparse_tag == TAG_MOUNT_POINT)
    }
}

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

fn u64_at(b: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(o..o + 8)?.try_into().ok()?))
}

/// Undoes the update-sequence protection: the last two bytes of every 512-byte sector were
/// replaced on disk by a sequence number, and the originals live in the update sequence array.
fn apply_fixups(rec: &mut [u8]) -> Option<()> {
    let usa_off = usize::from(u16_at(rec, 4)?);
    let usa_count = usize::from(u16_at(rec, 6)?);
    if usa_count < 2 || usa_off + usa_count * 2 > rec.len() || (usa_count - 1) * SECTOR > rec.len()
    {
        return None;
    }
    let seq = [rec[usa_off], rec[usa_off + 1]];
    for i in 1..usa_count {
        let end = i * SECTOR;
        if rec[end - 2..end] != seq {
            return None; // torn write
        }
        let (a, b) = (rec[usa_off + i * 2], rec[usa_off + i * 2 + 1]);
        rec[end - 2] = a;
        rec[end - 1] = b;
    }
    Some(())
}

/// Parses one record in place (fixups are applied to `rec`). Returns `None` for unused,
/// malformed or non-`FILE` records.
pub(crate) fn parse(rec: &mut [u8], number: u64) -> Option<Record> {
    if rec.get(..4)? != b"FILE" {
        return None;
    }
    apply_fixups(rec)?;
    let flags = u16_at(rec, 0x16)?;
    if flags & 1 == 0 {
        return None; // not in use
    }
    let base_ref = u64_at(rec, 0x20)? & REF_MASK;
    let mut out = Record {
        base: if base_ref == 0 { number } else { base_ref },
        is_base: base_ref == 0,
        is_dir: flags & 2 != 0,
        ..Record::default()
    };
    let used = usize::try_from(u32_at(rec, 0x18)?).ok()?.min(rec.len());
    let mut off = usize::from(u16_at(rec, 0x14)?);
    let mut name_rank = 0u8;

    while off + 16 <= used {
        let ty = u32_at(rec, off)?;
        if ty == ATTR_END {
            break;
        }
        let len = usize::try_from(u32_at(rec, off + 4)?).ok()?;
        if len < 16 || len % 8 != 0 || off + len > used {
            return None;
        }
        let attr = &rec[off..off + len];
        let non_resident = attr[8] != 0;
        let name_len = attr[9];
        match ty {
            ATTR_STANDARD_INFORMATION if !non_resident => {
                let c = content(attr)?;
                // Modified time and the attribute flags.
                let modified = i64::try_from(u64_at(c, 8)?).ok()?;
                out.mtime = modified / 10_000_000 - FILETIME_UNIX_DELTA;
                out.attrs = u32_at(c, 32)?;
            }
            ATTR_FILE_NAME if !non_resident && out.is_base => {
                let c = content(attr)?;
                let namespace = *c.get(0x41)?;
                // Prefer Win32 (1) and Win32+DOS (3) over POSIX (0); never use DOS-only (2).
                let rank = match namespace {
                    1 | 3 => 2,
                    0 => 1,
                    _ => 0,
                };
                if rank > name_rank {
                    let n = usize::from(*c.get(0x40)?);
                    let raw = c.get(0x42..0x42 + n * 2)?;
                    let units: Vec<u16> = raw
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|p| u16::from_le_bytes(*p))
                        .collect();
                    out.name = Some(String::from_utf16_lossy(&units).into_boxed_str());
                    out.parent = u64_at(c, 0)? & REF_MASK;
                    name_rank = rank;
                }
            }
            ATTR_DATA if name_len == 0 => {
                if non_resident {
                    if u64_at(attr, 0x10)? == 0 {
                        out.has_data = true;
                        out.size = u64_at(attr, 0x30)?;
                        let compressed = u16_at(attr, 0x22)? != 0;
                        out.allocated = if compressed && len >= 0x48 {
                            u64_at(attr, 0x40)?
                        } else {
                            u64_at(attr, 0x28)?
                        };
                    }
                } else {
                    out.has_data = true;
                    out.size = u64::from(u32_at(attr, 0x10)?);
                    out.allocated = 0; // resident data lives inside the record
                }
            }
            ATTR_REPARSE_POINT if !non_resident => {
                out.reparse_tag = u32_at(content(attr)?, 0)?;
            }
            _ => {}
        }
        off += len;
    }
    Some(out)
}

/// Resident attribute content slice.
fn content(attr: &[u8]) -> Option<&[u8]> {
    let len = usize::try_from(u32_at(attr, 0x10)?).ok()?;
    let off = usize::from(u16_at(attr, 0x14)?);
    attr.get(off..off.checked_add(len)?)
}

/// Decodes a runlist (mapping pairs) into `(lcn, cluster_count)` runs. Sparse runs are skipped
/// because the MFT itself is never sparse.
pub(crate) fn decode_runs(mut pairs: &[u8]) -> Option<Vec<(u64, u64)>> {
    let mut runs = Vec::new();
    let mut lcn: i64 = 0;
    while let Some((&header, rest)) = pairs.split_first() {
        if header == 0 {
            break;
        }
        let len_size = usize::from(header & 0x0F);
        let off_size = usize::from(header >> 4);
        if len_size == 0 || len_size > 8 || off_size > 8 || rest.len() < len_size + off_size {
            return None;
        }
        let mut count = 0u64;
        for (i, b) in rest[..len_size].iter().enumerate() {
            count |= u64::from(*b) << (8 * i);
        }
        if off_size > 0 {
            let mut delta = 0i64;
            for (i, b) in rest[len_size..len_size + off_size].iter().enumerate() {
                delta |= i64::from(*b) << (8 * i);
            }
            if rest[len_size + off_size - 1] & 0x80 != 0 && off_size < 8 {
                delta |= -1i64 << (8 * off_size); // sign-extend
            }
            lcn = lcn.checked_add(delta)?;
            runs.push((u64::try_from(lcn).ok()?, count));
        }
        pairs = &rest[len_size + off_size..];
    }
    Some(runs)
}

/// Extracts the runs of the unnamed non-resident `$DATA` attribute of the `$MFT` record.
pub(crate) fn mft_runs(rec: &mut [u8]) -> Option<Vec<(u64, u64)>> {
    if rec.get(..4)? != b"FILE" {
        return None;
    }
    apply_fixups(rec)?;
    let used = usize::try_from(u32_at(rec, 0x18)?).ok()?.min(rec.len());
    let mut off = usize::from(u16_at(rec, 0x14)?);
    while off + 16 <= used {
        let ty = u32_at(rec, off)?;
        if ty == ATTR_END {
            break;
        }
        let len = usize::try_from(u32_at(rec, off + 4)?).ok()?;
        if len < 16 || off + len > used {
            return None;
        }
        let attr = &rec[off..off + len];
        if ty == ATTR_DATA && attr[8] != 0 && attr[9] == 0 {
            let start = usize::from(u16_at(attr, 0x20)?);
            return decode_runs(attr.get(start..)?);
        }
        off += len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const REC: usize = 1024;

    fn put16(b: &mut [u8], o: usize, v: u16) {
        b[o..o + 2].copy_from_slice(&v.to_le_bytes());
    }
    fn put32(b: &mut [u8], o: usize, v: u32) {
        b[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn put64(b: &mut [u8], o: usize, v: u64) {
        b[o..o + 8].copy_from_slice(&v.to_le_bytes());
    }

    /// Builds a record with `attrs` appended, then protects it with an update sequence.
    fn build(flags: u16, base: u64, attrs: &[Vec<u8>]) -> Vec<u8> {
        let mut r = vec![0u8; REC];
        r[..4].copy_from_slice(b"FILE");
        put16(&mut r, 4, 0x30); // usa offset
        put16(&mut r, 6, 3); // usa count: sequence + 2 sectors
        put16(&mut r, 0x14, 0x38);
        put16(&mut r, 0x16, flags);
        put64(&mut r, 0x20, base);
        let mut off = 0x38;
        for a in attrs {
            r[off..off + a.len()].copy_from_slice(a);
            off += a.len();
        }
        put32(&mut r, off, ATTR_END);
        put32(&mut r, 0x18, u32::try_from(off + 8).unwrap());
        // Update sequence: value 0xBEEF, original sector tails stored in the array.
        put16(&mut r, 0x30, 0xBEEF);
        for i in 1..3 {
            let end = i * SECTOR;
            let (a, b) = (r[end - 2], r[end - 1]);
            r[0x30 + i * 2] = a;
            r[0x30 + i * 2 + 1] = b;
            put16(&mut r, end - 2, 0xBEEF);
        }
        r
    }

    fn resident(ty: u32, content: &[u8]) -> Vec<u8> {
        let len = (0x18 + content.len() + 7) & !7;
        let mut a = vec![0u8; len];
        put32(&mut a, 0, ty);
        put32(&mut a, 4, u32::try_from(len).unwrap());
        put32(&mut a, 0x10, u32::try_from(content.len()).unwrap());
        put16(&mut a, 0x14, 0x18);
        a[0x18..0x18 + content.len()].copy_from_slice(content);
        a
    }

    fn std_info(attrs: u32, unix_secs: i64) -> Vec<u8> {
        let mut c = vec![0u8; 48];
        put64(
            &mut c,
            8,
            u64::try_from((unix_secs + FILETIME_UNIX_DELTA) * 10_000_000).unwrap(),
        );
        put32(&mut c, 32, attrs);
        resident(ATTR_STANDARD_INFORMATION, &c)
    }

    fn file_name(parent: u64, name: &str, namespace: u8) -> Vec<u8> {
        let units: Vec<u16> = name.encode_utf16().collect();
        let mut c = vec![0u8; 0x42 + units.len() * 2];
        put64(&mut c, 0, parent | (7 << 48)); // sequence bits must be ignored
        c[0x40] = u8::try_from(units.len()).unwrap();
        c[0x41] = namespace;
        for (i, u) in units.iter().enumerate() {
            put16(&mut c, 0x42 + i * 2, *u);
        }
        resident(ATTR_FILE_NAME, &c)
    }

    fn data_nonresident(real: u64, alloc: u64) -> Vec<u8> {
        let mut a = vec![0u8; 0x48];
        put32(&mut a, 0, ATTR_DATA);
        put32(&mut a, 4, 0x48);
        a[8] = 1;
        put64(&mut a, 0x28, alloc);
        put64(&mut a, 0x30, real);
        a
    }

    #[test]
    fn parses_a_file_and_prefers_the_long_name() {
        let attrs = [
            std_info(attr::HIDDEN, 1_700_000_000),
            file_name(5, "REPORT~1.TXT", 2),
            file_name(5, "report final.txt", 1),
            data_nonresident(10_000, 12_288),
        ];
        let mut rec = build(1, 0, &attrs);
        let r = parse(&mut rec, 40).expect("record");
        assert_eq!(r.name.as_deref(), Some("report final.txt"));
        assert_eq!((r.parent, r.size, r.allocated), (5, 10_000, 12_288));
        assert_eq!(r.mtime, 1_700_000_000);
        assert!(r.is_base && !r.is_dir && r.attrs & attr::HIDDEN != 0);
        assert_eq!(r.base, 40);
    }

    #[test]
    fn resident_data_has_no_allocation_and_dirs_are_flagged() {
        let attrs = [
            file_name(5, "tiny", 3),
            resident(ATTR_DATA, &[1, 2, 3, 4, 5]),
        ];
        let mut rec = build(1, 0, &attrs);
        let r = parse(&mut rec, 9).unwrap();
        assert_eq!((r.size, r.allocated), (5, 0));
        let mut dir = build(3, 0, &[file_name(5, "d", 1)]);
        assert!(parse(&mut dir, 10).unwrap().is_dir);
    }

    #[test]
    fn unused_torn_and_foreign_records_are_skipped() {
        let mut unused = build(0, 0, &[file_name(5, "x", 1)]);
        assert!(parse(&mut unused, 1).is_none());
        let mut torn = build(1, 0, &[file_name(5, "x", 1)]);
        torn[SECTOR - 2] ^= 0xFF;
        assert!(parse(&mut torn, 1).is_none());
        let mut junk = vec![0u8; REC];
        assert!(parse(&mut junk, 1).is_none());
    }

    #[test]
    fn extension_record_points_at_its_base() {
        let mut rec = build(1, 77, &[data_nonresident(4, 4096)]);
        let r = parse(&mut rec, 90).unwrap();
        assert!(!r.is_base);
        assert_eq!((r.base, r.size), (77, 4));
    }

    #[test]
    fn symlink_tag_makes_a_link() {
        let mut tag = vec![0u8; 8];
        put32(&mut tag, 0, TAG_SYMLINK);
        let attrs = [
            std_info(attr::REPARSE_POINT, 0),
            file_name(5, "l", 1),
            resident(ATTR_REPARSE_POINT, &tag),
        ];
        let mut rec = build(1, 0, &attrs);
        assert!(parse(&mut rec, 3).unwrap().is_link());
    }

    #[test]
    fn runlist_decodes_relative_signed_offsets() {
        // 0x21: 1-byte length, 2-byte offset. 8 clusters at LCN 0x1000, then 4 at 0x1000-0x10.
        let pairs = [0x21, 0x08, 0x00, 0x10, 0x11, 0x04, 0xF0, 0x00];
        assert_eq!(decode_runs(&pairs).unwrap(), vec![(0x1000, 8), (0x0FF0, 4)]);
        assert!(decode_runs(&[0x21, 0x08]).is_none());
    }
}
