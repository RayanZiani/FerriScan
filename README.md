# FerriScan

A fast, private, open-source disk space analyzer for **Windows, Linux and macOS**.

FerriScan shows what is actually eating your drives: an instant overview of every volume,
a zoomable treemap, sunburst and sorted tables, extension breakdowns, and the largest files —
all rendered from a Rust scanning engine built for tens of millions of entries.

> **Status: early alpha.** The engine, the shell and the first views are in place and tested
> (see [`docs/ROADMAP.md`](docs/ROADMAP.md)). File operations, sunburst, the extension view and
> settings UI are the next milestones.

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

## What works today

| Area | State |
| --- | --- |
| Volume list with capacity, free space, filesystem and hot refresh | working |
| Parallel scan of a volume or folder, live progress, cancellation | working |
| Tree table with sort, breadcrumb navigation, keyboard rows | working |
| Treemap (canvas, squarified, zoom into folders) | working |
| Search, filters, extensions and sunburst | engine ready, UI pending |
| Move / copy / trash / permanent delete | planning + same-volume move; rest pending |
| Settings persistence with migration and unknown-key preservation | working |
| Light / dark / high-contrast themes, 12 locales | tokens and English; translations pending |

## Quick start

Prerequisites: Rust (stable 1.98, pinned by `rust-toolchain.toml`), Node.js 22+, and the
platform WebView dependencies (WebView2 on Windows, `libwebkit2gtk-4.1` on Linux, Xcode CLT on
macOS).

```bash
git clone https://github.com/<org>/ferriscan.git
cd ferriscan
npm --prefix apps/desktop install
npm --prefix apps/desktop run tauri dev
```

Headless scan of a folder:

```bash
cargo run -p ferriscan-cli -- scan . --top 20
cargo run -p ferriscan-cli -- volumes
```

## Verify

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm --prefix apps/desktop run check
npm --prefix apps/desktop run test
```

## Documentation

- [`docs/SPEC.md`](docs/SPEC.md) — product and technical specification (the source of truth)
- [`docs/FILE_ARCHITECTURE.md`](docs/FILE_ARCHITECTURE.md) — repository layout, crates, module contracts
- [`docs/ROADMAP.md`](docs/ROADMAP.md) — milestones, tasks, exit criteria
- [`docs/PRIVACY.md`](docs/PRIVACY.md) — what is stored, where, and what is never collected
- [`docs/adr/`](docs/adr/) — architecture decision records
- [`AGENTS.md`](AGENTS.md) — engineering workflow and rules for human and AI contributors

## License

MIT. See [`LICENSE`](LICENSE).
