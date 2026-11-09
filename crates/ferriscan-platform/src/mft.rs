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

/// Per-file facts, indexed by record number.
#[derive(Clone, Copy, Default)]
struct Node {
    flags: u16,
    size: u64,
    allocated: u64,
    mtime: i64,
}

/// One name of a file: a hard-linked file has several, each listed in its own directory.
#[derive(Clone, Copy)]
struct Link {
    record: u32,
    parent: u32,
    name_off: u32,
    name_len: u16,
}

/// Collects parsed records (in any order) and freezes them into an [`MftIndex`].
#[derive(Default)]
pub(super) struct IndexBuilder {
    nodes: Vec<Node>,
    links: Vec<Link>,
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
        // Extension records belong to their base file: their names are links of the base.
        let owner = if rec.is_base {
            number
        } else {
            let Ok(base) = u32::try_from(rec.base) else {
                return;
            };
            if rec.has_data {
                self.extensions.push((base, rec.size, rec.allocated));
            }
            base
        };
        for (parent, name) in &rec.links {
            let (Ok(name_off), Ok(name_len), Ok(parent)) = (
                u32::try_from(self.names.len()),
                u16::try_from(name.len()),
                u32::try_from(*parent),
            ) else {
                continue;
            };
            self.names.push_str(name);
            self.links.push(Link {
                record: owner,
                parent,
                name_off,
                name_len,
            });
        }
        if !rec.is_base {
            return;
        }
        let mut flags = PRESENT;
        for (cond, bit) in [
            (rec.is_dir, DIR),
            (rec.attrs & attr::HIDDEN != 0, HIDDEN),
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
            flags,
            size: rec.size,
            allocated: rec.allocated,
            mtime: rec.mtime,
        };
    }

    /// Resolves extension records and builds the parent -> links table.
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
        let nodes = &self.nodes;
        let reachable = |l: &Link| {
            l.record != ROOT
                && l.record >= FIRST_USER_RECORD
                && (l.parent as usize) < len
                && nodes[l.parent as usize].flags & (PRESENT | DIR) == PRESENT | DIR
        };
        // Group links by parent directory (counting sort).
        let mut start = vec![0u32; len + 1];
        for l in self.links.iter().filter(|l| reachable(l)) {
            start[l.parent as usize + 1] += 1;
        }
        for i in 0..len {
            start[i + 1] += start[i];
        }
        let mut fill = start.clone();
        let mut children = vec![
            Link {
                record: 0,
                parent: 0,
                name_off: 0,
                name_len: 0
            };
            start[len] as usize
        ];
        for l in self.links.iter().filter(|l| reachable(l)) {
            let slot = &mut fill[l.parent as usize];
            children[*slot as usize] = *l;
            *slot += 1;
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
    children: Vec<Link>,
}

impl MftIndex {
    fn name(&self, l: &Link) -> &str {
        let o = l.name_off as usize;
        &self.names[o..o + usize::from(l.name_len)]
    }

    fn children_of(&self, dir: u32) -> &[Link] {
        match (
            self.start.get(dir as usize),
            self.start.get(dir as usize + 1),
        ) {
            (Some(&a), Some(&b)) => &self.children[a as usize..b as usize],
            _ => &[],
        }
    }

    /// Number of directory entries indexed (a hard-linked file counts once per name).
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
            cur = self
                .children_of(cur)
                .iter()
                .find(|l| {
                    self.nodes[l.record as usize].flags & DIR != 0
                        && self.name(l).eq_ignore_ascii_case(&name)
                })?
                .record;
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

    /// Like [`Self::open`] but only for a whole-drive root and without reporting why it failed.
    ///
    /// This is what the app calls: any `None` means "use the directory walk".
    #[cfg(windows)]
    #[must_use]
    pub fn try_open(root: &Path) -> Option<Self> {
        let is_drive_root = root
            .components()
            .all(|c| matches!(c, Component::Prefix(_) | Component::RootDir));
        if !is_drive_root {
            return None;
        }
        match Self::open(root) {
            Ok(reader) => Some(reader),
            Err(err) => {
                tracing::debug!(error = %err, "MFT fast scan unavailable");
                None
            }
        }
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
        for l in idx.children_of(dir) {
            let n = &idx.nodes[l.record as usize];
            let name = idx.name(l);
            let kind = if n.flags & LINK != 0 {
                EntryKind::Symlink
            } else if n.flags & DIR != 0 {
                EntryKind::Dir
            } else {
                EntryKind::File
            };
            out.push(RawEntry {
                name: name.into(),
                kind,
                size: n.size,
                allocated: n.allocated,
                mtime: n.mtime,
                hidden: n.flags & HIDDEN != 0 || name.starts_with('.'),
                readonly: n.flags & READONLY != 0,
                system: n.flags & SYSTEM != 0,
                shared: false,
                key: u64::from(l.record),
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
            links: vec![(parent, name.into())],
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
    fn hard_links_appear_in_every_directory() {
        let mut b = IndexBuilder::default();
        b.add(5, &rec(5, ".", true, 0));
        b.add(20, &rec(5, "d", true, 0));
        let mut f = rec(5, "one.dll", false, 7);
        f.links.push((20, "two.dll".into()));
        b.add(30, &f);
        let r = MftReader::new(b.finish());
        let (mut root, mut d) = (Vec::new(), Vec::new());
        r.read_dir_keyed(Path::new(""), 5, &mut root).unwrap();
        r.read_dir_keyed(Path::new(""), 20, &mut d).unwrap();
        assert!(root.iter().any(|e| &*e.name == "one.dll" && e.size == 7));
        assert_eq!(d.len(), 1);
        assert_eq!((&*d[0].name, d[0].size), ("two.dll", 7));
    }

    #[test]
    fn names_stored_in_an_extension_record_belong_to_the_base() {
        let mut b = IndexBuilder::default();
        b.add(5, &rec(5, ".", true, 0));
        // Directory 20 has no name in its base record; the name lives in extension 99.
        let mut base = rec(5, "", true, 0);
        base.links.clear();
        b.add(20, &base);
        let mut ext = rec(5, "Docs", true, 0);
        ext.is_base = false;
        ext.base = 20;
        ext.has_data = false;
        b.add(99, &ext);
        b.add(21, &rec(20, "x.txt", false, 1));
        let r = MftReader::new(b.finish());
        assert_eq!(r.root_key(Path::new("C:\Docs")), 20);
        let mut out = Vec::new();
        r.read_dir_keyed(Path::new(""), 20, &mut out).unwrap();
        assert_eq!(out.len(), 1);
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
