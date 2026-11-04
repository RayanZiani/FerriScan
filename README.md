# FerriScan

A fast, private, open-source disk space analyzer for **Windows, Linux and macOS**.

FerriScan shows what is actually eating your drives: an instant overview of every volume,
a zoomable treemap, sunburst and sorted tables, extension breakdowns, and the largest files —
all rendered from a Rust scanning engine built for tens of millions of entries.

> **Status: pre-alpha.** The repository currently contains the specification, the architecture,
> the roadmap and the engineering agent setup. Code is being built milestone by milestone
> (see [`docs/ROADMAP.md`](docs/ROADMAP.md)).

## Why FerriScan

- **Fast.** Parallel scanner, compact index (~56 bytes per node), immutable snapshots, no
  file content reads. Budgets are measured in CI, not promised.
- **Private by design.** No account, no telemetry, no network access, no upload. Settings are
  a local JSON file; scan results live in memory and are gone when you close the app.
- **Safe file operations.** Delete to trash, delete permanently, move, copy and rename always
  go through an explicit plan and confirmation. Moves are journaled and undoable.
- **Native feel.** Follows the Windows light/dark theme and accent color, high-contrast mode,
  system locale, and adapts to Linux and macOS conventions.
- **Open.** MIT licensed, readable architecture, no hidden services.

## What it does (target v1.0)

| Area | Capability |
| --- | --- |
| Volumes | List drives, capacity, free space, filesystem, removable/local/network |
| Scan | Full-volume or single-folder scan, cancellable, non-blocking, progress |
| Views | Tree table (sortable/filterable), treemap, sunburst, extension bars, largest files |
| Search | Instant name search inside the current snapshot |
| Actions | Open, reveal in file manager, move, copy, rename, delete to trash, permanent delete |
| Safety | Pre-flight plans, conflict detection, cross-volume handling, undo for moves |
| Privacy | Local-only settings, opt-in local cache, zero telemetry |
| Languages | English, French, German, Spanish, Italian, Portuguese (BR), Dutch, Polish, Russian, Turkish, Japanese, Simplified Chinese |
| Themes | System / light / dark, Windows accent color, high contrast, reduced motion |

## Quick start (development)

Prerequisites: Rust (stable 1.90+, pinned by `rust-toolchain.toml`), Node.js 22+, and the platform WebView dependencies
(WebView2 on Windows, `libwebkit2gtk-4.1` on Linux, Xcode CLT on macOS).

```bash
git clone https://github.com/<org>/ferriscan.git
cd ferriscan
npm --prefix apps/desktop install
npm --prefix apps/desktop run tauri dev
```

Headless scan of a folder (works before the UI exists):

```bash
cargo run -p ferriscan-cli -- scan . --top 20
```

## Documentation

- [`docs/SPEC.md`](docs/SPEC.md) — product and technical specification (the source of truth)
- [`docs/FILE_ARCHITECTURE.md`](docs/FILE_ARCHITECTURE.md) — repository layout, crates, module contracts
- [`docs/ROADMAP.md`](docs/ROADMAP.md) — milestones, tasks, exit criteria
- [`docs/PRIVACY.md`](docs/PRIVACY.md) — what is stored, where, and what is never collected
- [`docs/adr/`](docs/adr/) — architecture decision records
- [`AGENTS.md`](AGENTS.md) — engineering workflow and rules for human and AI contributors

## License

MIT. See [`LICENSE`](LICENSE) (added in milestone M0).
