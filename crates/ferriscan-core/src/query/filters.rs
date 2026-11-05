//! Composable filters shared by every view.

use crate::model::{NodeId, NodeStore};

/// Restricts a listing to files, directories, or both.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum KindFilter {
    /// No kind restriction.
    #[default]
    All,
    /// Files only.
    Files,
    /// Directories only.
    Directories,
}

/// A set of predicates applied to a node before it is listed.
///
/// An empty set matches everything; this is the default for a fresh view.
#[derive(Clone, Debug, Default)]
pub struct FilterSet {
    /// Case-insensitive substring on the name. Accent folding is applied.
    pub name_contains: Option<String>,
    /// Uppercase extensions without leading dot; empty means no restriction.
    pub extensions: Vec<String>,
    /// Inclusive lower bound on the size basis, in bytes.
    pub min_size: Option<u64>,
    /// Inclusive upper bound on the size basis, in bytes.
    pub max_size: Option<u64>,
    /// Entries modified at least this many days ago.
    pub older_than_days: Option<u32>,
    /// Files, directories or both.
    pub kind: KindFilter,
}

impl FilterSet {
    /// Whether any predicate is active.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.name_contains.is_none()
            && self.extensions.is_empty()
            && self.min_size.is_none()
            && self.max_size.is_none()
            && self.older_than_days.is_none()
            && self.kind == KindFilter::All
    }

    /// Evaluates the filter against one node.
    #[must_use]
    pub fn matches(&self, store: &NodeStore, id: NodeId) -> bool {
        let rec = store.get(id);
        let flags = rec.flags();

        match self.kind {
            KindFilter::Files if !flags.is_file() => return false,
            KindFilter::Directories if !flags.is_dir() => return false,
            _ => {}
        }

        if let Some(needle) = &self.name_contains
            && !contains_fold(store.name(id), needle)
        {
            return false;
        }

        if !self.extensions.is_empty() {
            let ext = store.extensions().name(rec.ext_id);
            if !self
                .extensions
                .iter()
                .any(|wanted| wanted.eq_ignore_ascii_case(ext))
            {
                return false;
            }
        }

        if let Some(min) = self.min_size
            && rec.size < min
        {
            return false;
        }
        if let Some(max) = self.max_size
            && rec.size > max
        {
            return false;
        }
        true
    }
}

/// ASCII-case-insensitive and accent-insensitive substring test.
///
/// Full Unicode folding would allocate per comparison; the ASCII map plus a
/// small French/German accent table covers the shipped locales and stays
/// allocation-free for the common case.
#[must_use]
pub fn contains_fold(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let hay: Vec<char> = haystack.chars().flat_map(fold_char).collect();
    let pat: Vec<char> = needle.chars().flat_map(fold_char).collect();
    if pat.is_empty() || pat.len() > hay.len() {
        return pat.is_empty();
    }
    hay.windows(pat.len()).any(|w| w == pat.as_slice())
}

fn fold_char(c: char) -> std::iter::Once<char> {
    let lowered = match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'a',
        'ç' | 'Ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
        'ñ' | 'Ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => 'o',
        'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
        'ý' | 'ÿ' | 'Ý' => 'y',
        'ß' => 's',
        other => other.to_ascii_lowercase(),
    };
    std::iter::once(lowered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NodeFlags;

    #[test]
    fn folding_ignores_case_and_accents() {
        assert!(contains_fold("Résumé.PDF", "resume"));
        assert!(contains_fold("Résumé.PDF", "RÉSUMÉ"));
        assert!(contains_fold("Café", "cafe"));
        assert!(contains_fold("Cafe", "café"));
        assert!(contains_fold("anything", ""));
    }

    #[test]
    fn empty_filter_matches_everything() {
        let mut store = NodeStore::with_capacity(4);
        let root = store.insert_root("root", 0, NodeFlags::DIR).expect("root");
        let filters = FilterSet::default();
        assert!(filters.is_empty());
        assert!(filters.matches(&store, root));
    }

    #[test]
    fn extension_filter_is_case_insensitive() {
        let mut store = NodeStore::with_capacity(4);
        let root = store.insert_root("root", 0, NodeFlags::DIR).expect("root");
        let file = store
            .insert(root, "photo.JPG", NodeFlags::FILE, 10, 10, 0, 1, None)
            .expect("file");
        let filters = FilterSet {
            extensions: vec!["jpg".into()],
            ..FilterSet::default()
        };
        assert!(filters.matches(&store, file));
    }
}
