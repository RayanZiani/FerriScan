//! Developer tasks. `cargo xtask bump [patch|minor|major]` edits the workspace version;
//! `cargo xtask build` bumps the patch number, then builds the release app;
//! `cargo xtask i18n-check` verifies message keys used in code against the locale files.

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const MANIFEST: &str = "Cargo.toml";

/// Returns `version` with the requested part incremented and the lower parts reset.
fn bump(version: &str, part: &str) -> Result<String, String> {
    let n: Vec<u32> = version
        .split('.')
        .map(|p| p.parse().map_err(|_| version.to_string()))
        .collect::<Result<_, _>>()?;
    let [major, minor, patch] = n[..] else {
        return Err(version.to_string());
    };
    Ok(match part {
        "major" => format!("{}.0.0", major + 1),
        "minor" => format!("{major}.{}.0", minor + 1),
        "patch" => format!("{major}.{minor}.{}", patch + 1),
        other => return Err(format!("unknown part `{other}` (patch|minor|major)")),
    })
}

/// Rewrites the first `version = "x.y.z"` line (the `[workspace.package]` one) and returns the new version.
fn bump_manifest(part: &str) -> Result<String, String> {
    let text = fs::read_to_string(MANIFEST).map_err(|e| e.to_string())?;
    let line = text
        .lines()
        .find(|l| l.starts_with("version = \""))
        .ok_or("no workspace version")?;
    let old = line
        .trim_start_matches("version = \"")
        .trim_end_matches('"');
    let new = bump(old, part)?;
    fs::write(
        MANIFEST,
        text.replacen(line, &format!("version = \"{new}\""), 1),
    )
    .map_err(|e| e.to_string())?;
    Ok(new)
}

const LOCALES: &str = "crates/ferriscan-i18n/locales";
const SOURCES: [&str; 2] = ["apps/desktop/src", "crates/ferriscan-i18n/src"];

/// `key -> placeholders` for one `.ftl` file (`key = value`, `{name}` placeholders).
fn parse_ftl(text: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    for line in text.lines().filter(|l| !l.trim_start().starts_with('#')) {
        let Some((key, value)) = line.split_once(" = ") else {
            continue;
        };
        let mut holders = BTreeSet::new();
        let mut rest = value;
        while let Some(open) = rest.find('{') {
            let Some(close) = rest[open..].find('}') else {
                break;
            };
            holders.insert(rest[open + 1..open + close].trim().to_string());
            rest = &rest[open + close + 1..];
        }
        out.insert(key.trim().to_string(), holders);
    }
    out
}

/// Keys defined more than once in one file (the later definition silently wins).
fn duplicate_keys(text: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once(" = "))
        .map(|(k, _)| k.trim().to_string())
        .filter(|k| !seen.insert(k.clone()))
        .collect()
}

/// String literals that look like message keys (`area.name`, lowercase, dots and underscores).
fn literal_keys(source: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for piece in source.split('"').skip(1).step_by(2) {
        let looks_like_key = piece.contains('.')
            && !piece.starts_with('.')
            && !piece.ends_with('.')
            && piece
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'_');
        if looks_like_key {
            keys.push(piece.to_string());
        }
    }
    keys
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Every non-English locale must have the English keys and the same placeholders, and every key
/// written as a literal in the UI code must exist in English.
fn i18n_check() -> Result<(), String> {
    let read = |p: &Path| fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()));
    let en_text = read(&Path::new(LOCALES).join("en.ftl"))?;
    let en = parse_ftl(&en_text);
    let mut problems = Vec::new();
    for key in duplicate_keys(&en_text) {
        problems.push(format!("en.ftl: duplicate key `{key}`"));
    }

    for entry in fs::read_dir(LOCALES).map_err(|e| e.to_string())?.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "ftl") || path.ends_with("en.ftl") {
            continue;
        }
        let text = read(&path)?;
        let other = parse_ftl(&text);
        for key in duplicate_keys(&text) {
            problems.push(format!("{}: duplicate key `{key}`", path.display()));
        }
        for key in other.keys().filter(|k| !en.contains_key(*k)) {
            problems.push(format!("{}: key `{key}` is not in en.ftl", path.display()));
        }
        for (key, holders) in &en {
            match other.get(key) {
                None => problems.push(format!("{}: missing key `{key}`", path.display())),
                Some(h) if h != holders => {
                    problems.push(format!(
                        "{}: placeholders differ for `{key}`",
                        path.display()
                    ));
                }
                Some(_) => {}
            }
        }
    }

    let mut files = Vec::new();
    for dir in SOURCES {
        rust_files(Path::new(dir), &mut files);
    }
    for file in files {
        for key in literal_keys(&read(&file)?) {
            // Only report keys of areas the catalog owns.
            let area = key.split('.').next().unwrap_or("");
            let known_area = en.keys().any(|k| k.split('.').next() == Some(area));
            if known_area && !en.contains_key(&key) {
                problems.push(format!("{}: unknown key `{key}`", file.display()));
            }
        }
    }

    if problems.is_empty() {
        println!("i18n-check: {} keys, ok", en.len());
        Ok(())
    } else {
        Err(problems.join(
            "
",
        ))
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("bump") => println!("{}", bump_manifest(&args.next().unwrap_or("patch".into()))?),
        Some("build") => {
            println!("building v{}", bump_manifest("patch")?);
            let ok = Command::new("cargo")
                .args(["build", "--release", "-p", "ferriscan-app"])
                .status()
                .map_err(|e| e.to_string())?
                .success();
            if !ok {
                return Err("build failed (version already bumped)".into());
            }
        }
        Some("i18n-check") => i18n_check()?,
        _ => {
            return Err(
                "usage: cargo xtask <bump [patch|minor|major] | build | i18n-check>".into(),
            );
        }
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("xtask: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{bump, duplicate_keys, literal_keys, parse_ftl};

    #[test]
    fn ftl_placeholders_and_literal_keys_are_found() {
        let ftl = parse_ftl(
            "# c
a.b = {x} of {y}
c.d = plain
",
        );
        assert_eq!(ftl["a.b"].len(), 2);
        assert!(ftl["c.d"].is_empty());
        assert_eq!(
            duplicate_keys(
                "a = 1
b = 2
a = 3
"
            ),
            ["a"]
        );
        assert_eq!(literal_keys(r#"t("a.b"); "Not.key"; "x""#), ["a.b"]);
    }

    #[test]
    fn bumps_reset_lower_parts() {
        assert_eq!(bump("0.1.10", "patch").unwrap(), "0.1.11");
        assert_eq!(bump("0.1.10", "minor").unwrap(), "0.2.0");
        assert_eq!(bump("0.9.4", "major").unwrap(), "1.0.0");
        assert!(bump("0.1", "patch").is_err());
        assert!(bump("0.1.0", "x").is_err());
    }
}
