//! Deterministic in-memory filesystem used by tests and benchmarks.
//!
//! `FakeFs` lets the engine be exercised without touching a disk, which makes
//! scan tests reproducible and CI-safe on any machine. It is compiled behind the
//! `test-fs` feature for downstream crates, and always available to this crate's
//! own tests.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::fs::entry::{DirReader, EntryKind, FsError, RawEntry};
use crate::scan::errors::ErrorCode;

/// One scripted directory or error.
#[derive(Clone, Debug)]
enum Node {
    Dir(BTreeMap<String, Node>),
    File(u64),
    Symlink(String),
    Failing(ErrorCode),
}

/// An in-memory directory tree.
#[derive(Clone, Debug)]
pub struct FakeFs {
    root: Node,
    device: Option<u64>,
}

impl FakeFs {
    /// Creates an empty tree whose root is a directory.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: Node::Dir(BTreeMap::new()),
            device: None,
        }
    }

    /// Sets the device id reported for every path.
    #[must_use]
    pub fn with_device(mut self, device: u64) -> Self {
        self.device = Some(device);
        self
    }

    /// Adds a file at `path` (relative to the root).
    pub fn file(&mut self, path: &str, size: u64) -> &mut Self {
        self.insert(path, &Node::File(size))
    }

    /// Adds a directory at `path`.
    pub fn dir(&mut self, path: &str) -> &mut Self {
        self.insert(path, &Node::Dir(BTreeMap::new()))
    }

    /// Adds a symlink at `path`.
    pub fn symlink(&mut self, path: &str, target: &str) -> &mut Self {
        self.insert(path, &Node::Symlink(target.to_string()))
    }

    /// Makes `path` fail when read with the given code.
    pub fn failing(&mut self, path: &str, code: ErrorCode) -> &mut Self {
        self.insert(path, &Node::Failing(code))
    }

    fn insert(&mut self, path: &str, node: &Node) -> &mut Self {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        let mut cur = &mut self.root;
        for (i, part) in parts.iter().enumerate() {
            let Node::Dir(children) = cur else {
                return self;
            };
            let last = i + 1 == parts.len();
            if last {
                children.insert((*part).to_string(), node.clone());
                return self;
            }
            cur = children
                .entry((*part).to_string())
                .or_insert_with(|| Node::Dir(BTreeMap::new()));
        }
        self
    }

    fn resolve(&self, path: &Path) -> Option<&Node> {
        let mut cur = &self.root;
        for component in path.components() {
            let std::path::Component::Normal(name) = component else {
                continue;
            };
            let Node::Dir(children) = cur else {
                return None;
            };
            cur = children.get(name.to_str()?)?;
        }
        Some(cur)
    }

    /// Builds a tree with `files` files spread over `dirs` directories, useful for
    /// benchmarks. Each file is 1 KiB by default.
    #[must_use]
    pub fn synthetic(files: usize, dirs: usize, file_size: u64) -> Self {
        let mut fs = Self::new();
        let dirs = dirs.max(1);
        for d in 0..dirs {
            fs.dir(&format!("d{d:05}"));
        }
        for f in 0..files {
            let d = f % dirs;
            fs.file(&format!("d{d:05}/f{f:07}.bin"), file_size);
        }
        fs
    }

    /// Paths of every directory in the tree, for tests that need to attach errors.
    #[must_use]
    pub fn directories(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        collect(&self.root, Path::new(""), &mut out);
        out
    }
}

fn collect(node: &Node, prefix: &Path, out: &mut Vec<PathBuf>) {
    if let Node::Dir(children) = node {
        out.push(prefix.to_path_buf());
        for (name, child) in children {
            collect(child, &prefix.join(name), out);
        }
    }
}

impl Default for FakeFs {
    fn default() -> Self {
        Self::new()
    }
}

impl DirReader for FakeFs {
    fn read_dir(&self, dir: &Path, out: &mut Vec<RawEntry>) -> Result<(), FsError> {
        let node = self.resolve(dir).ok_or_else(|| {
            FsError::new(ErrorCode::NotFound, format!("{} not found", dir.display()))
        })?;
        match node {
            Node::Dir(children) => {
                for (name, child) in children {
                    out.push(to_entry(name, child));
                }
                Ok(())
            }
            Node::Failing(code) => Err(FsError::new(*code, "scripted failure")),
            _ => Err(FsError::new(
                ErrorCode::Invalid,
                format!("{} is not a directory", dir.display()),
            )),
        }
    }

    fn device_id(&self, _path: &Path) -> Option<u64> {
        self.device
    }
}

/// Maps a scripted node to the entry the scanner sees.
///
/// A directory that fails to read is still listed as a directory: the failure is
/// reported when the scanner tries to descend into it, exactly like a real
/// permission error.
#[allow(clippy::match_same_arms)]
fn to_entry(name: &str, node: &Node) -> RawEntry {
    match node {
        Node::Dir(_) => RawEntry::dir(name),
        Node::File(size) => RawEntry::file(name, *size),
        Node::Symlink(target) => RawEntry {
            name: name.into(),
            kind: EntryKind::Symlink,
            size: target.len() as u64,
            allocated: 0,
            mtime: 0,
            hidden: name.starts_with('.'),
            readonly: false,
            system: false,
            shared: false,
        },
        Node::Failing(_) => RawEntry::dir(name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_root_and_nested_directories() {
        let mut fs = FakeFs::new();
        fs.dir("a/b").file("a/b/x.txt", 10).file("y.bin", 5);
        let mut out = Vec::new();
        fs.read_dir(Path::new(""), &mut out).expect("root");
        let names: Vec<_> = out.iter().map(|e| e.name.as_ref()).collect();
        assert_eq!(names, vec!["a", "y.bin"]);

        out.clear();
        fs.read_dir(Path::new("a/b"), &mut out).expect("nested");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].size, 10);
    }

    #[test]
    fn scripted_failures_are_reported() {
        let mut fs = FakeFs::new();
        fs.failing("locked", ErrorCode::PermissionDenied);
        let err = fs
            .read_dir(Path::new("locked"), &mut Vec::new())
            .expect_err("must fail");
        assert_eq!(err.code, ErrorCode::PermissionDenied);
    }

    #[test]
    fn synthetic_tree_has_expected_size() {
        let fs = FakeFs::synthetic(100, 10, 1024);
        assert_eq!(fs.directories().len(), 11);
    }
}
