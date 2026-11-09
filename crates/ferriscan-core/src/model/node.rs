//! Core value types: node identifiers, flags and the flat node record.

use std::fmt;

/// Sentinel used for "no node" in `u32` link fields.
pub const NONE: u32 = u32::MAX;

/// Index of a node inside a [`crate::model::NodeStore`].
///
/// `NodeId` is deliberately not a pointer: it survives store growth, is stable
/// for the lifetime of a snapshot, and costs 4 bytes when stored in a view.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(u32);

impl NodeId {
    /// The absent node, linking the end of a sibling chain.
    pub const NONE: Self = Self(NONE);

    /// Wraps a raw index. Only [`crate::model::NodeStore`] may mint real ids.
    #[must_use]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// The underlying index.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Whether this is [`NodeId::NONE`].
    #[must_use]
    pub const fn is_none(self) -> bool {
        self.0 == NONE
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_none() {
            f.write_str("NodeId(NONE)")
        } else {
            write!(f, "NodeId({})", self.0)
        }
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_none() {
            f.write_str("none")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

/// Attribute bits stored alongside each node.
///
/// Kept as a `u16` so [`NodeRecord`] stays at 56 bytes; there is no need for a
/// general bitset library here.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct NodeFlags(u16);

impl NodeFlags {
    /// Directory entry.
    pub const DIR: Self = Self(1 << 0);
    /// Regular file.
    pub const FILE: Self = Self(1 << 1);
    /// Symbolic link or Windows reparse point.
    pub const SYMLINK: Self = Self(1 << 2);
    /// Hidden by OS convention (dotfile on Unix, attribute on Windows).
    pub const HIDDEN: Self = Self(1 << 3);
    /// System-critical attribute.
    pub const SYSTEM: Self = Self(1 << 4);
    /// Read-only attribute.
    pub const READONLY: Self = Self(1 << 5);
    /// Additional hard link exists; only counted once on Unix.
    pub const SHARED: Self = Self(1 << 6);
    /// Compressed storage (NTFS, btrfs).
    pub const COMPRESSED: Self = Self(1 << 7);
    /// Encrypted storage (NTFS EFS, APFS).
    pub const ENCRYPTED: Self = Self(1 << 8);
    /// The entry could not be fully read; size is a lower bound.
    pub const ERROR: Self = Self(1 << 9);

    /// Empty flag set.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Raw bits, for serialization.
    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Builds from raw bits, dropping unknown ones.
    #[must_use]
    pub const fn from_bits_truncate(bits: u16) -> Self {
        Self(bits)
    }

    /// Whether every bit of `other` is present.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Sets the bits of `other`.
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Clears the bits of `other`.
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    /// Directory test.
    #[must_use]
    pub const fn is_dir(self) -> bool {
        self.contains(Self::DIR)
    }

    /// Regular-file test.
    #[must_use]
    pub const fn is_file(self) -> bool {
        self.contains(Self::FILE)
    }

    /// Symlink test.
    #[must_use]
    pub const fn is_symlink(self) -> bool {
        self.contains(Self::SYMLINK)
    }

    /// Hidden test.
    #[must_use]
    pub const fn is_hidden(self) -> bool {
        self.contains(Self::HIDDEN)
    }
}

impl std::ops::BitOr for NodeFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for NodeFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl fmt::Debug for NodeFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut list = f.debug_list();
        if self.contains(Self::DIR) {
            list.entry(&"dir");
        }
        if self.contains(Self::FILE) {
            list.entry(&"file");
        }
        if self.contains(Self::SYMLINK) {
            list.entry(&"symlink");
        }
        if self.contains(Self::HIDDEN) {
            list.entry(&"hidden");
        }
        if self.contains(Self::SYSTEM) {
            list.entry(&"system");
        }
        if self.contains(Self::READONLY) {
            list.entry(&"readonly");
        }
        if self.contains(Self::SHARED) {
            list.entry(&"shared");
        }
        if self.contains(Self::COMPRESSED) {
            list.entry(&"compressed");
        }
        if self.contains(Self::ENCRYPTED) {
            list.entry(&"encrypted");
        }
        if self.contains(Self::ERROR) {
            list.entry(&"error");
        }
        list.finish()
    }
}

/// One entry in the flat index. Exactly 56 bytes; see ADR-0002.
///
/// Field order is chosen so the three `u64` fields land on 8-byte boundaries
/// without padding. Do not reorder without updating the size assertion.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NodeRecord {
    /// Logical size in bytes. Directories hold the aggregated value after finalize.
    pub size: u64,
    /// On-disk size in bytes.
    pub allocated: u64,
    /// Modification time, Unix seconds; `0` means unknown.
    pub mtime: i64,
    /// Offset of the name inside the store's name arena.
    pub name_off: u32,
    /// Parent node, or [`NONE`].
    pub parent: u32,
    /// First child, or [`NONE`].
    pub first_child: u32,
    /// Last child, or [`NONE`]; enables O(1) append during the scan.
    pub last_child: u32,
    /// Next sibling, or [`NONE`].
    pub next_sibling: u32,
    /// Direct child count.
    pub child_count: u32,
    /// Name length in bytes (valid UTF-8, so <= `65_535`).
    pub name_len: u16,
    /// Interned extension id; `0` means "no extension".
    pub ext_id: u16,
    /// Depth from the scan root (root is 0).
    pub depth: u16,
    /// [`NodeFlags`] bits.
    pub flags: u16,
}

const _: () = assert!(std::mem::size_of::<NodeRecord>() == 56);

impl NodeRecord {
    /// Parses the flag bits.
    #[must_use]
    pub const fn flags(&self) -> NodeFlags {
        NodeFlags::from_bits_truncate(self.flags)
    }

    /// The parent id.
    #[must_use]
    pub const fn parent(&self) -> NodeId {
        NodeId(self.parent)
    }

    /// The first child id.
    #[must_use]
    pub const fn first_child(&self) -> NodeId {
        NodeId(self.first_child)
    }

    /// The next sibling id.
    #[must_use]
    pub const fn next_sibling(&self) -> NodeId {
        NodeId(self.next_sibling)
    }

    /// Direct child count.
    #[must_use]
    pub const fn child_count(&self) -> u32 {
        self.child_count
    }
}

impl fmt::Debug for NodeRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeRecord")
            .field("size", &self.size)
            .field("allocated", &self.allocated)
            .field("mtime", &self.mtime)
            .field("parent", &NodeId(self.parent))
            .field("child_count", &self.child_count)
            .field("name_len", &self.name_len)
            .field("ext_id", &self.ext_id)
            .field("depth", &self.depth)
            .field("flags", &self.flags())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_is_56_bytes() {
        assert_eq!(std::mem::size_of::<NodeRecord>(), 56);
    }

    #[test]
    fn flags_compose() {
        let mut f = NodeFlags::DIR | NodeFlags::HIDDEN;
        assert!(f.is_dir());
        assert!(f.is_hidden());
        assert!(!f.is_file());
        f.remove(NodeFlags::HIDDEN);
        assert!(!f.is_hidden());
    }

    #[test]
    fn node_id_none_is_sentinel() {
        assert!(NodeId::NONE.is_none());
        assert_eq!(NodeId::NONE.raw(), u32::MAX);
        assert_eq!(NodeId::from_raw(7).raw(), 7);
    }
}
