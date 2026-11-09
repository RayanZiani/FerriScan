//! Developer tasks. `cargo xtask bump [patch|minor|major]` edits the workspace version;
//! `cargo xtask build` bumps the patch number, then builds the release app.

use std::{env, fs, process::Command};

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
        _ => return Err("usage: cargo xtask <bump [patch|minor|major] | build>".into()),
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
    use super::bump;

    #[test]
    fn bumps_reset_lower_parts() {
        assert_eq!(bump("0.1.10", "patch").unwrap(), "0.1.11");
        assert_eq!(bump("0.1.10", "minor").unwrap(), "0.2.0");
        assert_eq!(bump("0.9.4", "major").unwrap(), "1.0.0");
        assert!(bump("0.1", "patch").is_err());
        assert!(bump("0.1.0", "x").is_err());
    }
}
