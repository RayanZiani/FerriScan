# Changelog

All notable changes to FerriScan are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Product specification, file architecture, roadmap, privacy inventory and platform matrix.
- Architecture decision records 0001–0004 (Tauri/Svelte, node store, action tokens, no telemetry).
- Engineering agent setup and project conventions.
- Native GPUI desktop app: volume list, fast scan with progress and cancel, size table, treemap, selection actions (open, show in folder, copy path, move to recycle bin), settings, toasts (ADR-0005).
- Windows bulk directory reader (about 7x faster scans) and recycle-bin / open-path shell calls.
- Opt-in NTFS MFT fast scan (ADR-0006): relaunch as administrator, then whole-drive scans read the MFT (about 6 s for 2.8M entries on `C:\` versus 15-40 s for the directory walk). `ferriscan scan <path> --mft` in an elevated terminal does the same.
- Versioning: `cargo xtask bump [patch|minor|major]` and `cargo xtask build` (patch bump per build).
- High-contrast theme (every pair at least 7:1, covered by a test), window rectangle restored between runs, scan thread count in settings, Windows executable icon.
- `cargo xtask i18n-check` (locale completeness, placeholder parity, unknown keys), enforced in CI.
- Rust message catalog (`ferriscan-i18n`) with English strings.

### Changed

- The UI moved from Tauri + Svelte to GPUI (ADR-0005 supersedes ADR-0001). No webview, no Node.js, no IPC layer.
- Rust toolchain pinned to 1.98.1 (required by GPUI).

### Fixed

### Security

## [0.0.0] — 2026-11-04

Initial repository: documentation and agent scaffolding only. No application code yet.
