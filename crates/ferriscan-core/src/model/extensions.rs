//! Extension interning and aggregate statistics.

use rustc_hash::FxHashMap;

/// Maps lowercase extensions to a small integer id used by [`crate::model::NodeRecord`].
///
/// Id `0` is reserved for "no extension", so a node without a suffix costs no
/// lookup at render time.
#[derive(Debug, Default)]
pub struct ExtensionIndex {
    ids: FxHashMap<Box<str>, u16>,
    names: Vec<Box<str>>,
}

impl ExtensionIndex {
    /// Creates an index that only knows about the reserved "none" entry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ids: FxHashMap::default(),
            names: vec![Box::from("")],
        }
    }

    /// Interns an extension (with or without leading dot) and returns its id.
    ///
    /// Returns `0` for an empty extension or when the dictionary is full, which
    /// keeps the scan running instead of failing on a pathological directory.
    pub fn intern(&mut self, ext: &str) -> u16 {
        let ext = ext.trim_start_matches('.');
        if ext.is_empty() {
            return 0;
        }
        let mut lower = String::with_capacity(ext.len());
        for ch in ext.chars() {
            for c in ch.to_lowercase() {
                lower.push(c);
            }
        }
        if let Some(&id) = self.ids.get(lower.as_str()) {
            return id;
        }
        let Ok(id) = u16::try_from(self.names.len()) else {
            return 0;
        };
        let boxed: Box<str> = lower.into_boxed_str();
        self.ids.insert(boxed.clone(), id);
        self.names.push(boxed);
        id
    }

    /// Name of an interned extension, or `""` for id `0`.
    #[must_use]
    pub fn name(&self, id: u16) -> &str {
        self.names.get(usize::from(id)).map_or("", |s| s)
    }

    /// Number of distinct extensions, excluding the reserved entry.
    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len().saturating_sub(1)
    }

    /// Whether no extension has been interned yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// All interned extensions, in id order.
    pub fn iter(&self) -> impl Iterator<Item = (u16, &str)> {
        self.names
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, name)| (u16::try_from(i).unwrap_or(0), name.as_ref()))
    }
}

/// Splits a file name into an extension candidate without allocating.
///
/// A name has an extension when it contains a dot that is not the first
/// character and not the last. `"archive.tar.gz"` yields `"gz"`.
#[must_use]
pub fn extension_of(name: &str) -> &str {
    match name.rfind('.') {
        Some(idx) if idx > 0 && idx + 1 < name.len() => &name[idx + 1..],
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_detection() {
        assert_eq!(extension_of("photo.JPG"), "JPG");
        assert_eq!(extension_of("archive.tar.gz"), "gz");
        assert_eq!(extension_of("Makefile"), "");
        assert_eq!(extension_of(".gitignore"), "");
        assert_eq!(extension_of("trailing."), "");
    }

    #[test]
    fn interning_is_case_insensitive() {
        let mut idx = ExtensionIndex::new();
        let a = idx.intern("JPG");
        let b = idx.intern(".jpg");
        assert_eq!(a, b);
        assert_eq!(idx.name(a), "jpg");
        assert_eq!(idx.len(), 1);
        assert_eq!(idx.intern(""), 0);
    }
}
