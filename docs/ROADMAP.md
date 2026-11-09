# FerriScan — Roadmap

Milestones are sequential; each has an owner role, a task list, and **exit criteria** that must
be green before the next milestone starts. Task IDs are stable and referenced by PR titles
(`feat(M3): T3.4 paged children query`).

**Definition of done** for every task: see `docs/SPEC.md` §17.3.

---

## Implementation status (after the GPUI cutover, ADR-0005)

Verified on Windows 11 with `cargo test --workspace`, `clippy --workspace --all-targets -D warnings`,
`cargo fmt --check` and `cargo deny check`. The Tauri/Svelte app and its npm checks no longer
exist. Linux and macOS builds of the GPUI app have not been run yet.

| Milestone | State | Notes |
| --- | --- | --- |
| M0 Foundation | **done** | workspace, CI workflows, toolchain, agents, icon |
| M1 Volumes | **mostly done** | Windows verified; Linux/macOS code written, not yet run in CI |
| M2 Scanner core | **core done** | store, controller, workers, errors, progress, aggregation, CLI; Windows bulk reader done (about 7x faster); Linux/macOS fast paths and benchmarks pending |
| M3 Query + table | **mostly done** | children paging, size table (virtualized), folder navigation, progress and cancel in the GPUI shell; search, filters and column sorting pending |
| M4 Visualizations | **started** | treemap done (hover, select, enter); sunburst, extensions, largest files pending |
| M5 Actions | **started** | plan validation, protected paths, same-volume move; copy/trash/delete/journal pending |
| M6 Settings/theme/i18n | **started** | persisted settings with migration, tokens, light/dark/high-contrast, English strings |
| M7–M12 | **not started** | performance hardening, a11y audit, packaging, beta, 1.0 |

Measured on a warm NVMe in a debug build: 13.6k entries in 335 ms (~40k entries/s). Release
budgets are still to be validated by T2.10.

---

## M0 — Foundation (week 1)

Owner: Core. Goal: a repository that builds, lints, tests and runs an empty window on all three
OSes.

| ID | Task | Notes |
| --- | --- | --- |
| T0.1 | Workspace `Cargo.toml`, pinned toolchain, rustfmt/clippy/deny config | `rust-toolchain.toml` stable |
| T0.2 | Create the six crates + `xtask`, each with a doc comment and one smoke test | no logic yet |
| T0.3 | ~~Tauri 2 + Svelte 5 + Vite 8 scaffold~~ GPUI app scaffold in `apps/desktop` | superseded by ADR-0005 |
| T0.4 | CI: fmt, clippy `-D warnings`, test matrix, build | GitHub Actions |
| T0.5 | `LICENSE` (MIT), `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, issue/PR templates | |
| T0.6 | ADR 0001 (superseded by 0005), ADR 0004 (no telemetry) accepted | |

**Exit:** `cargo test --workspace` and `cargo build --release -p ferriscan-app` succeed on Windows, Linux and macOS CI runners. Empty window opens with correct theme.

---

## M1 — Volumes (week 2)

Owner: Platform. Goal: the app lists real volumes on all platforms.

| ID | Task |
| --- | --- |
| T1.1 | `VolumeProvider` trait + `VolumeDto` + fake provider |
| T1.2 | Windows implementation (drive strings, type, label, fs, free space) incl. locked volumes |
| T1.3 | Linux implementation (`mountinfo`, `statvfs`, pseudo-fs filtering) |
| T1.4 | macOS implementation (`getmntinfo`, `statfs`) |
| T1.5 | `list_volumes` command + polling helper |
| T1.6 | Volume list UI: cards with capacity bar, kind icon, free/used, refresh |
| T1.7 | Unit tests per platform with recorded fixtures; integration test on real temp mounts |

**Exit:** on each OS, all local volumes appear with correct sizes (±1 block) and refresh on
plug/unplug within 5 s. No panics on a network volume that disappears.

---

## M2 — Scanner core (weeks 3–5)

Owner: Core. Goal: a fast, cancellable, testable scan producing a snapshot.

| ID | Task |
| --- | --- |
| T2.1 | `NodeStore` (SoA arena, name arena, insert/link, invariant checks) + property tests |
| T2.2 | `ScanOptions`, `ScanController` state machine, cancel token, generation |
| T2.3 | Worker pool: work-stealing directory queue, batched store insertion, no-alloc hot loop |
| T2.4 | Platform `DirReader` implementations (Windows bulk, Linux `statx`, macOS bulk) |
| T2.5 | Error sink with taxonomy and caps (`E_*` codes) |
| T2.6 | Progress atomics + 10 Hz coalesced reporter |
| T2.7 | Finalize: aggregation, histograms, extension index, largest-N heap |
| T2.8 | `FakeFs` + fixture generator (1M files, 200k siblings, deep chain, unicode, loops) |
| T2.9 | `ferriscan-cli scan` with table/JSON output |
| T2.10 | Criterion benchmarks + `bench/budgets.toml` + CI gate |
| T2.11 | Hardlink/symlink/sparse/compressed semantics per SPEC §8.5 |

**Exit:** 1M synthetic entries scanned in budget on CI hardware; cancel ≤ 250 ms; invariants
hold under `proptest`; CLI output matches golden snapshots.

---

## M3 — Query layer and tree table (weeks 5–7)

Owner: Core + UI. Goal: browse a scanned tree efficiently.

| ID | Task |
| --- | --- |
| T3.1 | `QueryService`: children paging with sort/filter, 200k-sibling index |
| T3.2 | Breadcrumb reconstruction + path cache |
| T3.3 | Search (substring, bounded, path strings) |
| T3.4 | Filter engine (name/ext/size/age/kind) shared with all views |
| T3.5 | DTOs + `specta` (or schema) generation + TS mirror + contract test |
| T3.6 | ~~IPC commands~~ direct engine calls from the app: snapshot meta, node, children, breadcrumb, search |
| T3.7 | TreeTable component with virtualization, resizable columns, multi-select |
| T3.8 | Toolbar, breadcrumbs, status bar, empty/loading/error states |
| T3.9 | Scan progress UI wired to events, cancel button, issues panel |
| T3.10 | Keyboard navigation + `Ctrl+F` search overlay |

**Exit:** 100k-entry scan browsable at 60 fps; 500-row page ≤ 10 ms P95; keyboard-complete
navigation; E2E journey "scan → expand → sort → search" passes.

---

## M4 — Visualizations (weeks 7–9)

Owner: UI. Goal: the product looks like FerriScan.

| ID | Task |
| --- | --- |
| T4.1 | Squarified treemap in Rust + determinism tests + 5k-node budget |
| T4.2 | Canvas treemap renderer with hit grid, hover tooltip, zoom/re-root |
| T4.3 | Sunburst layout + renderer |
| T4.4 | Extension bars view + cross-filtering |
| T4.5 | Largest files view |
| T4.6 | Overview cards + "top hogs" summary |
| T4.7 | Palette: categorical, color-blind safe, contrast-tested; legend |
| T4.8 | Selection sync across all views |

**Exit:** view switching never rescans; 5k-node layout within budget; all views share filters
and selection; accessible table alternative for each chart.

---

## M5 — Actions and safety (weeks 9–12)

Owner: Core (actions). Goal: the analyzer can safely change the disk.

| ID | Task |
| --- | --- |
| T5.1 | `plan_action` with conflict/permission/free-space/protected-path validation |
| T5.2 | HMAC confirm token + expiry + plan cache + TOCTOU re-validation |
| T5.3 | Move (same-volume rename, cross-volume copy→verify→delete), copy, rename |
| T5.4 | Trash + permanent delete with bottom-up traversal and symlink safety |
| T5.5 | Journal (write-ahead, atomic) + undo + retention |
| T5.6 | Action progress events and per-item report |
| T5.7 | UI: action dialog, plan summary, conflict resolver, delete confirmation with typed word |
| T5.8 | Open / reveal / copy path commands (argv only) |
| T5.9 | Snapshot patching after mutations (subtree re-read, parent recompute) |
| T5.10 | Table-driven safety tests: conflicts, TOCTOU, permission denied, busy file, ENOSPC |
| T5.11 | Export CSV/JSON of the current scope |

**Exit:** every destructive path requires a plan; undo succeeds for moves in 100 % of test
scenarios; zero data loss in the adversarial test suite; protected paths blocked on all OSes.

---

## M6 — Settings, theme, i18n (weeks 12–14)

Owner: UI + Core.

| ID | Task |
| --- | --- |
| T6.1 | `Settings` model, versioned JSON, atomic write, migration harness |
| T6.2 | Settings UI: appearance, language, units, scan defaults, excludes, privacy, about |
| T6.3 | Design tokens, light/dark/high-contrast, accent integration, reduced motion |
| T6.4 | OS theme/locale listeners wired to native window theme |
| T6.5 | Fluent integration, `en` complete, 11 other locales at ≥ 90 % |
| T6.6 | `xtask i18n-check` + literal-string lint |
| T6.7 | Number/date/byte formatting per locale |

**Exit:** settings survive restart and update; switching language/theme applies instantly with
no flicker; every shipped locale passes key checks; axe reports zero serious violations.

---

## M7 — Performance hardening (weeks 14–15)

Owner: Core.

| ID | Task |
| --- | --- |
| T7.1 | Profile a 5M-entry scan; remove top 10 allocators/locks |
| T7.2 | Adaptive concurrency for spinning disks; semaphore tuning |
| T7.3 | Zero-copy layout arrays (IPC payload trimming no longer applies) |
| T7.4 | Memory ceiling validation (≤ 96 B/entry) and arena growth strategy |
| T7.5 | Cold-start optimization (prefetch volumes, lazy layout) |
| T7.6 | Long-running stability test (10 consecutive scans, no leak) |

**Exit:** all budgets in SPEC §6.3 green on reference hardware; memory profile documented;
no unbounded growth over 10 scans (RSS delta < 5 %).

---

## M8 — Accessibility and polish (week 15)

| ID | Task |
| --- | --- |
| T8.1 | Full keyboard audit against the shortcut map |
| T8.2 | Screen-reader pass (NVDA, VoiceOver, Orca) with fixes |
| T8.3 | Focus management in modals, menus, dialogs |
| T8.4 | Empty/error/partial states copy and illustration |
| T8.5 | Micro-interactions within motion budget |
| T8.6 | Window geometry persistence, min sizes, multi-monitor |

**Exit:** WCAG 2.2 AA checklist signed; no keyboard trap; all states designed.

---

## M9 — Cross-platform validation (week 16)

| ID | Task |
| --- | --- |
| T9.1 | Windows 10 1809, 11, Server 2022; ARM64 VM |
| T9.2 | Ubuntu 22.04/24.04, Fedora 40+, Arch; Wayland and X11 |
| T9.3 | macOS 11 (Intel), 14/15 (Apple Silicon) |
| T9.4 | Network shares (SMB, NFS) read-only behavior |
| T9.5 | Removable media (USB) hot-plug during scan |
| T9.6 | `docs/PLATFORM_MATRIX.md` filled with verified results |

**Exit:** support matrix green or explicitly documented as unsupported with rationale.

---

## M10 — Packaging and release (weeks 16–17)

| ID | Task |
| --- | --- |
| T10.1 | Icons, installer metadata, AppStream, desktop entry |
| T10.2 | NSIS + MSI, signing pipeline (secret-gated) |
| T10.3 | AppImage/deb/rpm in CI, dependency docs |
| T10.4 | Universal dmg, notarization pipeline (secret-gated) |
| T10.5 | Release workflow: tag → build → checksums → GitHub Release → changelog |
| T10.6 | `SECURITY.md`, `CHANGELOG.md`, version in About |
| T10.7 | Fresh-machine install/launch/uninstall tests per OS |

**Exit:** downloadable artifacts for all platforms; install/uninstall clean; unsigned builds
still run with documented warnings.

---

## M11 — Beta (weeks 17–19)

| ID | Task |
| --- | --- |
| T11.1 | Public beta binaries + feedback template |
| T11.2 | Triage and fix P0/P1 from beta |
| T11.3 | Crash-free session target ≥ 99.5 % (local reports only) |
| T11.4 | Documentation site content: quick start, FAQ, privacy, shortcuts |
| T11.5 | Translation review by native speakers |

**Exit:** no open P0/P1; docs complete; privacy page published.

---

## M12 — 1.0 (week 20)

- Feature freeze, final QA matrix, signed artifacts, `1.0.0` tag, announcement.
- Out of scope confirmed: no telemetry, no account, no cloud.

---

## Post-1.0 backlog (unscheduled)

| Idea | Value | Notes |
| --- | --- | --- |
| Duplicate finder (size + partial hash + full hash) | High | Needs hashing pool and UI |
| USN journal / MFT fast scan on NTFS | Very high | Done as an opt-in elevated read-only MFT scan, ADR-0006 |
| Scheduled scans + history | Medium | Requires persisted snapshots and retention policy |
| Snapshot cache and diff between scans | Medium | "What grew since last week" |
| Archive inspection (zip/7z listing) | Medium | Memory and parsing risk |
| Privileged helper | Low | Security cost; ADR required |
| Plugin/scripting API | Low | Attack surface; ADR required |
| CLI `watch` mode | Low | Competes with `duf`/`ncdu` |

---

## Dependency-ordered task graph (compact)

```
M0 ──► M1 ──┐
      └────► M2 ──► M3 ──► M4 ──┐
                        └──────► M5 ──► M7 ──► M9 ──► M10 ──► M11 ──► M12
      M6 (settings/i18n) ──► M8 ─┘
```

M6 can start after M3 (needs DTOs); M8 needs M4 and M5 for complete surfaces. M5 can run in
parallel with M4 if two agents work simultaneously — they touch disjoint files except
`apps/desktop/src/shell.rs`.
