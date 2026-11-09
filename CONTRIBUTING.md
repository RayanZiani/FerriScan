# Contributing to FerriScan

Thanks for helping. FerriScan is a 100 % Rust desktop app built on GPUI. This guide is the short
version; `AGENTS.md` holds the full engineering rules and applies to humans and AI agents alike.

## Setup

Prerequisites: Rust 1.98.1 (pinned by `rust-toolchain.toml`) and the platform packages below.

| OS | Extra packages |
| --- | --- |
| Windows | Visual Studio Build Tools (C++ workload) |
| Linux | `libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libxcb1-dev libx11-xcb-dev libfontconfig-dev libvulkan-dev build-essential` |
| macOS | `xcode-select --install` |

```bash
git clone https://github.com/<org>/ferriscan.git
cd ferriscan
cargo run -p ferriscan-app
```

Headless engine check:

```bash
cargo run -p ferriscan-cli -- scan . --top 20
```

## Before you code

1. Read `docs/SPEC.md` for the behavior you are changing.
2. Read `docs/FILE_ARCHITECTURE.md` for the owning crate/module.
3. Pick a task from `docs/ROADMAP.md` (or open an issue to propose one).
4. For architectural changes, open an ADR draft (`docs/adr/0000-template.md`) first.

## Verify

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Windows, Linux and macOS are all first-class; a change that only works on one platform must say
so and update `docs/PLATFORM_MATRIX.md`.

## Versioning

`MAJOR.MINOR.PATCH`, single source in the root `Cargo.toml` (`[workspace.package]`).

- **PATCH**: every new build and every hotfix. `cargo xtask build` bumps it, then builds the release app (`0.1.10` → `0.1.11`).
- **MINOR** (`0.2.0`): a larger release with small features. `cargo xtask bump minor`.
- **1.0.0**: the first complete release. **2.0.0** is not planned yet.

Tag releases `vX.Y.Z` (the release workflow builds tags). `cargo xtask bump [patch|minor|major]` only edits the version.

## Conventions

- English everywhere: code, comments, commit messages, docs, issue titles.
- Conventional Commits (`feat(scan): …`), one logical change per PR.
- All UI strings go through `lib/i18n`; no literal text in components.
- Only `ferriscan-actions` may modify user files.
- No new network dependency. No telemetry. Ever, without an ADR and a product decision.

## Good first issues

Look for the `good first issue` label. Typical entry points: a new locale, a new fixture, a
small platform helper, a settings panel, a documentation page.

## Reporting bugs

Include: OS and version, FerriScan version (About dialog), what you did, what happened, and the
relevant log excerpt (Settings → About → Open log folder). Paths are redacted by default; you
can attach logs manually. Never paste a full directory listing of a personal disk.

## Security issues

Do not open a public issue. See `SECURITY.md`.
