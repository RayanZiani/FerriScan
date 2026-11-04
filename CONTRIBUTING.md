# Contributing to FerriScan

Thanks for helping. FerriScan is a Rust + Tauri + Svelte desktop app. This guide is the short
version; `AGENTS.md` holds the full engineering rules and applies to humans and AI agents alike.

## Setup

Prerequisites: Rust stable 1.90+ (pinned by `rust-toolchain.toml`), Node.js 22+, and platform WebView dependencies.

| OS | Extra packages |
| --- | --- |
| Windows | WebView2 runtime (evergreen), Visual Studio Build Tools (C++ workload) |
| Linux | `libwebkit2gtk-4.1-dev libgtk-3-dev libssl-dev librsvg2-dev build-essential` |
| macOS | `xcode-select --install` |

```bash
git clone https://github.com/<org>/ferriscan.git
cd ferriscan
npm --prefix apps/desktop install
npm --prefix apps/desktop run tauri dev
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
npm --prefix apps/desktop run check
npm --prefix apps/desktop run lint
npm --prefix apps/desktop run test
```

Windows, Linux and macOS are all first-class; a change that only works on one platform must say
so and update `docs/PLATFORM_MATRIX.md`.

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
