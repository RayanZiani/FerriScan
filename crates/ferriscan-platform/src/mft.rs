//! NTFS Master File Table reader (ADR-0006).
//!
//! The volume is read once, sequentially and read-only, into an in-memory index of every file.
//! [`MftReader`] then answers `read_dir_keyed` from that index, so the engine's scan loop
//! (filters, node store, aggregation) is unchanged and no per-directory open is needed.
//! Reading the volume requires elevation; the app falls back to the directory walk otherwise.

mod record;
#[cfg(windows)]
mod volume;

use std::path::{Component, Path};

use ferriscan_core::fs::{DirReader, EntryKind, FsError, RawEntry};

use record::{Record, attr};

/// Record number of the volume root directory.
const ROOT: u32 = 5;
/// Record numbers below this (except the root) are NTFS metafiles (`$MFT`, `$LogFile`, ...) that
/// directory enumeration never lists, so the index leaves them out for parity.
const FIRST_USER_RECORD: u32 = 16;

const PRESENT: u16 = 1;
const DIR: u16 = 2;
const HIDDEN: u16 = 4;
const SYSTEM: u16 = 8;
const READONLY: u16 = 16;
const LINK: u16 = 32;
const HAS_DATA: u16 = 64;

#[derive(Clone, Copy, Default)]
struct Node {
    name_off: u32,
    name_len: u16,
    flags: u16,
    parent: u32,
    size: u64,
    allocated: u64,
    mtime: i64,
}

/// Collects parsed records (in any order) and freezes them into an [`MftIndex`].
#[derive(Default)]
pub(super) struct IndexBuilder {
    nodes: Vec<Node>,
    names: String,
    /// `$DATA` facts found in extension records, applied to their base at the end.
    extensions: Vec<(u32, u64, u64)>,
}

impl IndexBuilder {
    /// Adds one parsed record. Records with absurd numbers are ignored.
    pub(super) fn add(&mut self, number: u64, rec: &Record) {
        let Ok(number) = u32::try_from(number) else {
            return;
        };
        if !rec.is_base {
            if rec.has_data
                && let Ok(base) = u32::try_from(rec.base)
            {
                self.extensions.push((base, rec.size, rec.allocated));
            }
            return;
        }
        let Some(name) = rec.name.as_deref() else {
            return; // no usable name: not reachable from any directory
        };
        let (Ok(name_off), Ok(name_len), Ok(parent)) = (
            u32::try_from(self.names.len()),
            u16::try_from(name.len()),
            u32::try_from(rec.parent),
        ) else {
            return;
        };
        self.names.push_str(name);
        let mut flags = PRESENT;
        for (cond, bit) in [
            (rec.is_dir, DIR),
            (
                rec.attrs & attr::HIDDEN != 0 || name.starts_with('.'),
                HIDDEN,
            ),
            (rec.attrs & attr::SYSTEM != 0, SYSTEM),
            (rec.attrs & attr::READONLY != 0, READONLY),
            (rec.is_link(), LINK),
            (rec.has_data, HAS_DATA),
        ] {
            if cond {
                flags |= bit;
            }
        }
        if self.nodes.len() <= number as usize {
            self.nodes.resize(number as usize + 1, Node::default());
        }
        self.nodes[number as usize] = Node {
            name_off,
            name_len,
            flags,
            parent,
            size: rec.size,
            allocated: rec.allocated,
            mtime: rec.mtime,
        };
    }

    /// Resolves extension records and builds the parent -> children table.
    pub(super) fn finish(mut self) -> MftIndex {
        for (base, size, allocated) in std::mem::take(&mut self.extensions) {
            if let Some(n) = self.nodes.get_mut(base as usize)
                && n.flags & PRESENT != 0
                && n.flags & HAS_DATA == 0
            {
                n.size = size;
                n.allocated = allocated;
                n.flags |= HAS_DATA;
            }
        }
        let len = self.nodes.len();
        let linked = |i: usize, n: &Node| {
            n.flags & PRESENT != 0
                && i != ROOT as usize
                && i >= FIRST_USER_RECORD as usize
                && (n.parent as usize) < len
                && self.nodes[n.parent as usize].flags & (PRESENT | DIR) == PRESENT | DIR
        };
        let mut start = vec![0u32; len + 1];
        for (i, n) in self.nodes.iter().enumerate() {
            if linked(i, n) {
                start[n.parent as usize + 1] += 1;
            }
        }
        for i in 0..len {
            start[i + 1] += start[i];
        }
        let mut fill = start.clone();
        let mut children = vec![0u32; start[len] as usize];
        for (i, n) in self.nodes.iter().enumerate() {
            if linked(i, n) {
                let slot = &mut fill[n.parent as usize];
                // `i < len <= u32::MAX` because record numbers were checked to fit u32.
                children[*slot as usize] = u32::try_from(i).unwrap_or(0);
                *slot += 1;
            }
        }
        MftIndex {
            nodes: self.nodes,
            names: self.names,
            start,
            children,
        }
    }
}

/// Immutable in-memory view of one volume's file table.
pub struct MftIndex {
    nodes: Vec<Node>,
    names: String,
    start: Vec<u32>,
    children: Vec<u32>,
}

impl MftIndex {
    fn name(&self, n: &Node) -> &str {
        let o = n.name_off as usize;
        &self.names[o..o + usize::from(n.name_len)]
    }

    fn children_of(&self, dir: u32) -> &[u32] {
        match (
            self.start.get(dir as usize),
            self.start.get(dir as usize + 1),
        ) {
            (Some(&a), Some(&b)) => &self.children[a as usize..b as usize],
            _ => &[],
        }
    }

    /// Number of files and directories indexed (excluding NTFS metafiles).
    #[must_use]
    pub fn len(&self) -> usize {
        self.children.len()
    }

    /// Whether the volume held no reachable entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    /// Finds a directory by walking `path` from the volume root (case-insensitive).
    fn lookup(&self, path: &Path) -> Option<u32> {
        let mut cur = ROOT;
        for comp in path.components() {
            let Component::Normal(name) = comp else {
                continue; // prefix and root dir are the volume itself
            };
            let name = name.to_string_lossy();
            cur = *self.children_of(cur).iter().find(|&&c| {
                let n = &self.nodes[c as usize];
                n.flags & DIR != 0 && self.name(n).eq_ignore_ascii_case(&name)
            })?;
        }
        Some(cur)
    }
}

/// [`DirReader`] backed by an [`MftIndex`].
pub struct MftReader {
    index: MftIndex,
}

impl MftReader {
    /// Wraps an index.
    #[must_use]
    pub const fn new(index: MftIndex) -> Self {
        Self { index }
    }

    /// Reads the MFT of the NTFS volume that holds `root` (a drive root such as `C:\`).
    ///
    /// # Errors
    /// Permission denied when the process is not elevated, `Unsupported` when the volume is not
    /// NTFS, or an I/O error. Callers fall back to the directory walk on any error.
    #[cfg(windows)]
    pub fn open(root: &Path) -> Result<Self, FsError> {
        volume::read_index(root).map(Self::new)
    }

    /// The underlying index.
    #[must_use]
    pub const fn index(&self) -> &MftIndex {
        &self.index
    }
}

impl DirReader for MftReader {
    fn read_dir(&self, dir: &Path, out: &mut Vec<RawEntry>) -> Result<(), FsError> {
        let key = self.index.lookup(dir).ok_or_else(|| {
            FsError::new(
                ferriscan_core::scan::errors::ErrorCode::NotFound,
                dir.display().to_string(),
            )
        })?;
        self.read_dir_keyed(dir, u64::from(key), out)
    }

    fn read_dir_keyed(
        &self,
        _dir: &Path,
        key: u64,
        out: &mut Vec<RawEntry>,
    ) -> Result<(), FsError> {
        let idx = &self.index;
        let Ok(dir) = u32::try_from(key) else {
            return Ok(()); // unknown directory: nothing to list
        };
        for &c in idx.children_of(dir) {
            let n = &idx.nodes[c as usize];
            let kind = if n.flags & LINK != 0 {
                EntryKind::Symlink
            } else if n.flags & DIR != 0 {
                EntryKind::Dir
            } else {
                EntryKind::File
            };
            out.push(RawEntry {
                name: idx.name(n).into(),
                kind,
                size: n.size,
                allocated: n.allocated,
                mtime: n.mtime,
                hidden: n.flags & HIDDEN != 0,
                readonly: n.flags & READONLY != 0,
                system: n.flags & SYSTEM != 0,
                shared: false,
                key: u64::from(c),
            });
        }
        Ok(())
    }

    fn root_key(&self, root: &Path) -> u64 {
        self.index.lookup(root).map_or(u64::MAX, u64::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(parent: u64, name: &str, dir: bool, size: u64) -> Record {
        Record {
            base: 0,
            is_base: true,
            is_dir: dir,
            parent,
            name: Some(name.into()),
            size,
            allocated: size,
            has_data: !dir,
            ..Record::default()
        }
    }

    fn sample() -> MftReader {
        let mut b = IndexBuilder::default();
        b.add(5, &rec(5, ".", true, 0));
        b.add(0, &rec(5, "$MFT", false, 999)); // metafile: must stay hidden
        b.add(20, &rec(5, "Users", true, 0));
        b.add(21, &rec(20, "a.txt", false, 10));
        let mut big = rec(20, "big.bin", false, 0);
        big.has_data = false; // its $DATA lives in an extension record
        b.add(22, &big);
        b.add(23, &rec(99, "orphan", false, 5)); // parent missing: unreachable
        let mut ext = rec(0, "", false, 4096);
        ext.is_base = false;
        ext.base = 22;
        b.add(30, &ext);
        MftReader::new(b.finish())
    }

    #[test]
    fn lists_children_hides_metafiles_and_orphans() {
        let r = sample();
        let mut out = Vec::new();
        r.read_dir_keyed(Path::new(""), u64::from(ROOT), &mut out)
            .unwrap();
        let names: Vec<_> = out.iter().map(|e| &*e.name).collect();
        assert_eq!(names, ["Users"]);
        out.clear();
        r.read_dir_keyed(Path::new(""), out_key(&r, "C:\\Users"), &mut out)
            .unwrap();
        let mut got: Vec<_> = out.iter().map(|e| (&*e.name, e.size)).collect();
        got.sort_unstable();
        assert_eq!(got, [("a.txt", 10), ("big.bin", 4096)]);
    }

    fn out_key(r: &MftReader, p: &str) -> u64 {
        r.root_key(Path::new(p))
    }

    #[test]
    fn path_lookup_is_case_insensitive_and_root_maps_to_five() {
        let r = sample();
        assert_eq!(r.root_key(Path::new("C:\\")), 5);
        assert_eq!(r.root_key(Path::new("C:\\users")), 20);
        let mut out = Vec::new();
        r.read_dir(Path::new("C:\\USERS"), &mut out).unwrap();
        assert_eq!(out.len(), 2);
        assert!(r.read_dir(Path::new("C:\\nope"), &mut out).is_err());
    }
}
