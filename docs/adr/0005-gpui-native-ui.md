# ADR-0005 — GPUI native UI replaces Tauri + Svelte

- **Status:** Accepted (2026-11-09, product owner). Cutover done: Tauri and Svelte are removed.
- **Date:** 2026-11-09
- **Deciders:** Product owner (request), `architect`, `ui-engineer`
- **Related:** supersedes ADR-0001; SPEC §6.3, §7, §10, §16, §17; FILE_ARCHITECTURE §7–§8, §10

## Context

ADR-0001 chose Tauri 2 + Svelte 5 for the shell. The product owner now wants FerriScan to be
100 % Rust and native: one language, one toolchain, no webview, no IPC boundary.

Today the UI is small (~3.4k lines of Svelte/TS plus ~700 lines of Tauri command glue), so the
cost of switching is lowest now. The engine crates (`ferriscan-core`, `-platform`, `-actions`,
`-persist`, `-i18n`) do not depend on Tauri and are unaffected.

What the webview gave us and must be replaced:

| Webview provided | GPUI replacement to build or verify |
| --- | --- |
| Text layout, IME, font fallback | GPUI text system (verify CJK/RTL/IME on Windows in the spike) |
| Virtualized list, tables | `uniform_list` + custom table widget |
| Canvas 2D treemap/sunburst | GPUI `canvas` / `paint_quad` / paths, same layout arrays from `core::layout` |
| CSS tokens, light/dark/high-contrast | Rust `Theme` struct, OS appearance observer, forced-colors mapping |
| Accessibility tree (UIA/ATK/AX) | **Open risk**: GPUI exposes no mature accessibility tree. SPEC §15 / `a11y-checklist.md` cannot be met by keyboard focus alone |
| Fluent via JS | `ferriscan-i18n` (Rust) drives all strings |

## Decision

Replace `apps/desktop/src-tauri` and `apps/desktop/src` with a single binary crate
`apps/desktop` (`ferriscan-app`) built on **GPUI**, pinned to an exact git revision of the Zed
repository (GPUI is not released as a stable crate). The UI calls the engine directly through
`Arc<Snapshot>` and typed Rust APIs. Node IDs remain the only handle the UI holds; paths are
resolved from the snapshot (AGENTS §2.5 stays true as a design rule, but is no longer a
security boundary because there is no untrusted renderer).

The migration is gated by a spike (M-G0). If its exit criteria fail, this ADR is rejected and
ADR-0001 stays in force.

## Consequences

### Positive

- One language and toolchain; no Node, npm, Vite, WebView2 or WebKitGTK dependency.
- No IPC serialization: removes the "IPC payload ≤ 120 KB" budget and the DTO/contract layer
  for the UI (`xtask contract`, `types.ts`, specta).
- Smaller attack surface: no webview, CSP or capability allowlist to maintain.
- GPU-rendered treemap/sunburst share the same process and memory as the snapshot.

### Negative

- GPUI is pre-1.0 and driven by Zed's needs; API breaks on every bump. We pin a revision and
  treat bumps as dedicated tasks.
- Accessibility, IME, high-contrast and screen-reader support must be verified or built.
  This may force a deviation from SPEC §15; any shortfall is recorded here, not hidden.
- Windows and Linux backends are less battle-tested than macOS; GPU/driver issues are
  possible (fallback story to be documented in `PLATFORM_MATRIX.md`).
- Contributors need Rust-only skills, but must learn GPUI's entity/context model.
- The Playwright + `tauri-driver` E2E strategy is lost; replaced by GPUI's `TestAppContext`
  tests plus screenshot checks via `scripts/`.
- The ~3.4k lines of Svelte/TS (including 24 vitest tests) are discarded and rewritten.

### Neutral

- Installer size and idle memory must be re-measured against NFR-7/NFR-8 in M-G0.
- `ferriscan-core::dto` stays for the CLI and persistence; it is no longer an IPC contract.

## Alternatives considered

| Option | Why rejected |
| --- | --- |
| Keep Tauri + Svelte (ADR-0001) | Not "100 % Rust native"; stays the baseline if the spike fails |
| egui / iced / Slint | Already evaluated in ADR-0001; the product owner specifically asked for GPUI |
| Dioxus (native renderer) | Younger renderer, still webview-based on desktop by default |
| GPUI + `gpui-component` crate | Faster start (table, inputs, theming) but a large third-party dependency; **decision deferred to the spike**, requires allowlist entry in FILE_ARCHITECTURE §2 |

## Plan

Vertical slices, workspace compiling at each step. Owners per AGENTS §10.

| Milestone | Content | Exit criteria |
| --- | --- | --- |
| **M-G0 Spike** | `apps/desktop-gpui` prototype beside Tauri: window, volume list, scan progress, 100k-row virtual list, 5k-node treemap | 60 fps on both lists/canvas; Windows build works; IME + keyboard focus verified; accessibility gap documented; installer size measured |
| **M-G1 Shell** | App state, theme (light/dark/high-contrast), i18n via `ferriscan-i18n`, settings persistence, toasts | parity with current `AppShell`, `VolumeList`, `StatusBar`, `Toolbar` |
| **M-G2 Views** | Tree table, treemap, overview; then sunburst, extensions, largest files | parity with current views; SPEC §6.3 budgets green (`cargo xtask budget`) |
| **M-G3 Actions UI** | Plan dialogs, delete confirm, conflict resolver on top of `ferriscan-actions` | ADR-0003 flow intact (token, TOCTOU tests) |
| **M-G4 Cutover** | Delete `src-tauri` and `src` (Svelte), remove Tauri/npm from CI and workspace, update docs | `cargo test --workspace`, clippy, deny green on all OSes |

Files changing in the docs (done with this ADR): `AGENTS.md`, `SPEC.md` §7.1, `FILE_ARCHITECTURE.md`
§7–§8, `adr/0001-tauri-svelte.md` (status). `SPEC` §10, §16, §17, `ROADMAP.md`, `TESTING.md`,
`PLATFORM_MATRIX.md`, `a11y-checklist.md` and CI workflows are updated in M-G0/M-G4 once the
spike fixes the real constraints.

## Follow-up

- [x] Run M-G0 and record measurements in this ADR (see results below).
- [x] M-G4 cutover: `apps/desktop` (Tauri, Svelte, npm) deleted; GPUI crate is now `apps/desktop`.
- [ ] Choose GPUI revision, and `gpui-component` yes/no; update FILE_ARCHITECTURE §2 allowlist.
- [ ] Decide the accessibility approach (e.g. AccessKit bridge) before M-G1.
- [ ] Rewrite `ROADMAP.md` M3–M6 UI tasks and `TESTING.md`.
- [ ] `security-reviewer` pass: removal of CSP/capabilities; decide the fate of the ADR-0003 confirm token now that the UI is in-process.
- [ ] Windows exe icon and installer (the Tauri icon assets were removed with the shell).
- [ ] High-contrast theme, keyboard parity, AccessKit verification, IME checks.

## Spike M-G0 results (2026-11-09, Windows 11, release build)

- GPUI pinned at Zed `ddb90e5e` (`gpui` + `gpui_platform`); **requires Rust 1.98.1**, so the
  workspace toolchain moved from 1.90 to 1.98.1 (clippy `-D warnings` clean after one
  `sort_by_key` fix).
- `apps/desktop-gpui`: volume list, 100k-row `uniform_list`, 5k-tile squarified treemap
  (`ferriscan-core::layout::squarify`) painted with `paint_quad`.
- Frame rate: steady ~60 fps with continuous redraw (vsync-capped, so headroom is unmeasured).
- Release binary: ~10 MiB (NFR-7 installer ≤ 15 MB looks achievable).
- GPUI now pulls AccessKit (`accesskit_windows`); real screen-reader behavior not yet verified.
- Not yet verified: IME/CJK input, keyboard focus traversal, high-contrast, Linux/macOS builds,
  hit-testing/hover on the treemap, cold-start time, idle memory.

### M-G0 closing notes

- Treemap hit-testing (hover), click selection and arrow/Home/End keyboard navigation work in
  `apps/desktop-gpui/src/bin/spike.rs` (shared cached layout for paint and hit-test); ~60 fps held.
  Interactive behavior was exercised only by launching the app, not by automated UI tests.
- Still unverified: IME/CJK, screen reader via AccessKit, high-contrast, Linux/macOS.

## M-G1 progress (shell)

- `ferriscan-i18n` now ships `Catalog` + `locales/en.ftl` (migrated from `index.ts`); partial
  locales fall back to English. Plurals are not implemented yet.
- `apps/desktop-gpui`: `theme.rs` (light/dark tokens, follows OS appearance), `format.rs`,
  `shell.rs` (toolbar, volume list with usage bars, status bar, async refresh, persisted
  settings loaded via `ferriscan-persist`). The spike lives in `src/bin/spike.rs`.
- Remaining for M-G1: toasts, settings panel, high-contrast theme, scan start/progress,
  window geometry persistence, locale loading for non-English.

### Scan speed (2026-11-09)

- Baseline `std::fs::read_dir` reader: `C:\Users\ZIANI` (2.15M entries) in 54 s (~39k entries/s).
- New Windows bulk reader (`ferriscan-platform/src/dir_reader/windows.rs`, 64 KiB
  `FILE_FULL_DIR_INFO` batches, real allocation size, no elevation): 7–8 s (~260–300k entries/s),
  same entry counts. Extrapolated 1 TB SSD (~3–4M entries): ~12–15 s.
- GPUI shell: Scan button per volume, background scan, 10 Hz progress and summary in the status
  bar, Cancel. Verified by build/launch only; the click path is not covered by automated tests.

### Scan speed, whole system drive (2026-11-09, correction)

The extrapolation above was optimistic. Measured on `C:\` (2.77M entries, 435k directories,
22 logical CPUs), release CLI:

- 40 s cold cache, 15-20 s warm. Threads: 4 -> 31 s, 8 -> 19 s, 16 -> 16 s, 32 -> 16 s.
- CPU time is 94 % kernel (159 s kernel vs 10 s user): user-space work (inserts, globs) is
  not the bottleneck. The node-store lock is held ~6 s in total.
- About half of the thread time is `CreateFileW` on directories (~65 us single-threaded,
  ~300 us under 16-thread contention), the rest is enumeration.
- Opening relative to the parent handle (`NtCreateFile` with `RootDirectory`) was measured
  at 65 us vs 67 us: no gain, so path resolution is not the cost.

Conclusion: a non-elevated directory walk is bounded around 150-200k entries/s here. The
15 s per TB figure that TreeSize reaches comes from reading the NTFS MFT, which requires an
elevated volume handle. That conflicts with the "no elevation in v1.0" rule of the SPEC and
is left as a decision for the maintainers.

## Cutover (M-G4, 2026-11-09)

- Removed: `apps/desktop` (Tauri shell, Svelte frontend, npm tooling), Node scripts, CI frontend
  / contract / e2e jobs, Tauri signing steps.
- Added: `apps/desktop` is now the GPUI crate `ferriscan-app` (workspace member, pedantic
  clippy clean). `cargo deny check` passes with the Zed git source allowed and five transitive
  "unmaintained" advisories ignored with reasons in `deny.toml`.
- The GPUI tree has the `http` types crate but no reqwest, hyper or TLS stack (ADR-0004 intact).
- Not yet reproduced: installer, app icon, code signing, Linux/macOS CI runs, `xtask`
  subcommands (still a stub).
