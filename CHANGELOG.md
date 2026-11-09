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
- Rust message catalog (`ferriscan-i18n`) with English strings.

### Changed

- The UI moved from Tauri + Svelte to GPUI (ADR-0005 supersedes ADR-0001). No webview, no Node.js, no IPC layer.
- Rust toolchain pinned to 1.98.1 (required by GPUI).

### Fixed

### Security

## [0.0.0] — 2026-11-04

Initial repository: documentation and agent scaffolding only. No application code yet.
