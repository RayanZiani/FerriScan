//! Protected path policy: operations that are refused unless explicitly forced.

use std::path::{Component, Path, PathBuf};

/// Why a path is protected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protection {
    /// The root of a volume.
    VolumeRoot,
    /// A system directory.
    SystemDirectory,
    /// The user's home directory itself (children are fine).
    HomeRoot,
    /// The `FerriScan` installation or data directory.
    SelfDirectory,
}

/// Result of the protection check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProtectedCheck {
    /// The path is safe to operate on.
    Allowed,
    /// The path is protected; the UI must show the reason and require an override.
    Blocked(Protection),
}

/// Checks whether `path` may be mutated.
///
/// The check normalizes components lexically (no symlink resolution, which would
/// require I/O and can race) and compares whole path prefixes, so `C:\Windows.old`
/// is not treated as `C:\Windows`.
#[must_use]
pub fn check(path: &Path) -> ProtectedCheck {
    let normalized = normalize(path);

    if is_volume_root(&normalized) {
        return ProtectedCheck::Blocked(Protection::VolumeRoot);
    }

    let mut roots: Vec<PathBuf> = system_roots();
    roots.extend(home_roots());
    for root in roots {
        if is_same_or_ancestor(&normalized, &root) && normalized == root {
            return ProtectedCheck::Blocked(if is_home(&root) {
                Protection::HomeRoot
            } else {
                Protection::SystemDirectory
            });
        }
        if is_inside(&normalized, &root) && root_protects_subtree(&root) {
            return ProtectedCheck::Blocked(Protection::SystemDirectory);
        }
    }

    ProtectedCheck::Allowed
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        out.push(path);
    }
    out
}

fn is_volume_root(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        let trimmed = text.trim_end_matches(['\\', '/']);
        trimmed.len() == 2 && trimmed.ends_with(':')
    }
    #[cfg(not(windows))]
    {
        path == Path::new("/")
    }
}

fn is_inside(path: &Path, root: &Path) -> bool {
    path != root && path.starts_with(root)
}

fn is_same_or_ancestor(path: &Path, root: &Path) -> bool {
    path == root || root.starts_with(path)
}

fn is_home(root: &Path) -> bool {
    home_roots().iter().any(|h| h == root)
}

/// Home is protected only at its own root: everything below is fair game, while
/// a system directory protects its whole subtree.
fn root_protects_subtree(root: &Path) -> bool {
    !is_home(root)
}

fn system_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    #[cfg(windows)]
    {
        for key in [
            "SystemRoot",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "ProgramData",
        ] {
            if let Some(value) = std::env::var_os(key) {
                roots.push(normalize(&PathBuf::from(value)));
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        for p in [
            "/System",
            "/Library",
            "/usr",
            "/bin",
            "/sbin",
            "/private",
            "/Applications",
        ] {
            roots.push(PathBuf::from(p));
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for p in [
            "/boot", "/etc", "/usr", "/bin", "/sbin", "/lib", "/lib64", "/proc", "/sys", "/dev",
            "/run",
        ] {
            roots.push(PathBuf::from(p));
        }
    }
    roots
}

fn home_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        roots.push(normalize(&PathBuf::from(home)));
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_paths_are_allowed() {
        let path = std::env::temp_dir().join("ferriscan-test").join("file.bin");
        assert_eq!(check(&path), ProtectedCheck::Allowed);
    }

    #[test]
    fn normalizes_parent_components() {
        let path = PathBuf::from("/tmp/a/../b");
        assert_eq!(normalize(&path), PathBuf::from("/tmp/b"));
    }

    #[cfg(unix)]
    #[test]
    fn root_is_blocked() {
        assert_eq!(
            check(Path::new("/")),
            ProtectedCheck::Blocked(Protection::VolumeRoot)
        );
    }

    #[cfg(unix)]
    #[test]
    fn system_prefixes_are_blocked() {
        assert!(matches!(
            check(Path::new("/etc/passwd")),
            ProtectedCheck::Blocked(_)
        ));
        assert!(matches!(
            check(Path::new("/usr/local/bin/tool")),
            ProtectedCheck::Blocked(_)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn similarly_named_prefix_is_not_blocked() {
        assert_eq!(check(Path::new("/etcetera/file")), ProtectedCheck::Allowed);
    }
}
