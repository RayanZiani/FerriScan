# FerriScan — File & Module Architecture

Companion to [`SPEC.md`](SPEC.md). This document defines the repository layout, the
responsibility of every crate and module, and the contracts between them. It is prescriptive:
new files go where this document says, or this document is amended first.

---

## 1. Repository layout

```
ferriscan/
├── AGENTS.md                       # engineering rules for humans + AI agents
├── README.md
├── LICENSE                          # MIT (added in M0)
├── Cargo.toml                       # workspace root
├── Cargo.lock                       # committed
├── rust-toolchain.toml              # pinned stable toolchain
├── rustfmt.toml
├── clippy.toml
├── deny.toml                        # cargo-deny policy
├── .editorconfig
├── .gitignore
├── opencode.jsonc                   # OpenCode project config (agents/commands wiring)
│
├── .opencode/
│   ├── agents/                      # specialised subagents (see AGENTS.md)
│   ├── commands/                    # reusable slash commands
│   └── skills/                      # project-local skills (spec, perf, i18n…)
│
├── docs/
│   ├── SPEC.md                      # product + technical source of truth
│   ├── FILE_ARCHITECTURE.md         # this file
│   ├── ROADMAP.md                   # milestones, backlog, exit criteria
│   ├── PRIVACY.md                   # data inventory
│   ├── PLATFORM_MATRIX.md           # per-OS feature/support matrix
│   ├── TESTING.md                   # test strategy in practice
│   └── adr/
│       ├── 0000-template.md
│       ├── 0001-tauri-svelte.md   # superseded by 0005
│       ├── 0002-node-store-layout.md
│       ├── 0003-action-plan-token.md
│       └── 0004-no-telemetry.md
│
├── crates/
│   ├── ferriscan-core/              # pure engine: model, index, scan orchestration, layouts
│   ├── ferriscan-platform/          # OS abstraction: volumes, dir readers, metadata, shell
│   ├── ferriscan-actions/           # plan/execute/undo for move/copy/rename/delete
│   ├── ferriscan-persist/           # settings, journals, optional snapshot cache
│   ├── ferriscan-i18n/              # locale detection + shared error-code catalogue
│   └── ferriscan-cli/               # headless scanner (dev tool + CI harness)
│
├── apps/
│   └── desktop/                     # GPUI application (package ferriscan-app, see §7)
│       └── src/
│
├── xtask/                           # cargo xtask: i18n-check, contract, budget, release
├── bench/                           # Criterion benches + fixture generator + budgets.toml
├── fixtures/                        # small committed trees; large ones generated
└── .github/workflows/               # ci.yml, nightly.yml, release.yml, security.yml
```

**Naming rules**

- Crates: `ferriscan-<area>`, directories kebab-case, lib names `ferriscan_<area>`.
- Rust files: `snake_case.rs`; one primary type per file; `mod.rs` forbidden (use
  `foo/mod.rs` → `foo.rs` + `foo/` instead).
- UI views and widgets: one `snake_case.rs` file per concern in `apps/desktop/src`.
- Locales: BCP-47 tags (`pt-BR`, `zh-CN`), lowercase in paths only for base languages.

---

## 2. Workspace and dependency graph

```toml
# Cargo.toml (shape)
[workspace]
resolver = "3"
members = ["crates/*", "apps/desktop", "xtask"]

[workspace.package]
edition = "2024"
rust-version = "1.98"
license = "MIT"
repository = "https://github.com/<org>/ferriscan"

[workspace.lints.rust]
unsafe_op_in_unsafe_fn = "deny"
missing_debug_implementations = "warn"

[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
unwrap_used = "warn"
expect_used = "warn"
```

Allowed dependencies (enforced by review + `cargo deny` bans on alternatives):

```
ferriscan-core      → (serde, thiserror, tracing, smallvec, rustc-hash, globset, rayon)
ferriscan-platform  → ferriscan-core, (windows-sys | nix/libc | core-foundation bindings), trash
ferriscan-actions   → ferriscan-core, ferriscan-platform, ferriscan-persist, (sha2, rand, hmac)
ferriscan-persist   → ferriscan-core, (serde, serde_json, directories)
ferriscan-i18n      → (unic-langid, sys-locale)
ferriscan-cli       → core, platform, actions, (clap, indicatif)
app (apps/desktop)  → core, platform, actions, persist, i18n + gpui, gpui_platform (pinned Zed revision)
```

**Hard rules**

1. `ferriscan-core` must not depend on any OS crate, UI crate, or `ferriscan-platform`.
   It depends on the traits in `core::fs` and is tested with `FakeFs`.
2. `unsafe` is only allowed in `ferriscan-platform` , each block preceded by a `// SAFETY:` comment.
3. No crate other than `ferriscan-actions` may mutate user files.
4. DTOs shared with the CLI and persistence live in `ferriscan-core::dto` (single source of truth).

---

## 3. `ferriscan-core` — engine

```
crates/ferriscan-core/src/
├── lib.rs                 # re-exports, crate docs, feature flags
├── model/
│   ├── node.rs            # NodeRecord, NodeId, flags, NodeRef accessors
│   ├── store.rs           # NodeStore: arena, name arena, insert/patch/iterate
│   ├── snapshot.rs        # Snapshot, SnapshotMeta, statuses
│   ├── extensions.rs      # ExtensionIndex: intern + totals
│   └── filters.rs         # FilterSet: name/ext/size/age/kind, compiled matcher
├── fs/
│   ├── mod.rs             # traits: VolumeProvider, DirReader, MetadataSource, Clock
│   ├── entry.rs           # RawEntry { name, kind, size, allocated, mtime, file_id, attrs }
│   └── fake.rs            # FakeFs implementation for tests/benches (feature `test-fs`)
├── scan/
│   ├── mod.rs             # ScanEngine, ScanOptions, ScanHandle
│   ├── controller.rs      # cancel token, generation, state machine
│   ├── worker.rs          # per-thread loop, steal queue, batching into store
│   ├── progress.rs        # atomic counters + coalescing reporter
│   ├── errors.rs          # ScanError, ErrorCode, error sink with caps
│   └── finalize.rs        # aggregation pass, histograms, largest-N heap
├── query/
│   ├── mod.rs             # QueryService over &Snapshot
│   ├── children.rs        # paging, sorting, filtering, child index
│   ├── search.rs          # substring search, bounded results
│   ├── breadcrumb.rs      # path reconstruction + caching
│   └── export.rs          # CSV/JSON row iterators
├── layout/
│   ├── mod.rs             # LayoutRequest → LayoutResult
│   ├── squarify.rs        # squarified treemap, deterministic
│   ├── treemap.rs         # depth/budget limiting, "N more" aggregation
│   └── sunburst.rs        # arc ranges, ring allocation
├── dto/
│   ├── mod.rs             # DTOs (CLI output, persistence) + serde attributes
│   ├── scan.rs, node.rs, action.rs, settings.rs, system.rs
├── util/
│   ├── units.rs           # byte formatting primitives (locale-neutral)
│   ├── path.rs            # logical path join/normalize (no OS calls)
│   └── bitset.rs          # compact selection/visited sets
└── tests/                 # unit-level integration inside the crate
```

Key invariants (asserted in debug, property-tested in CI):

- `I1` Every non-root node has a valid `parent`; `first_child`/`next_sibling` chains are acyclic.
- `I2` For every directory `d`: `d.size == Σ children.size` and `d.allocated == Σ children.allocated`
  after finalize; `d.child_count == number of children`.
- `I3` `depth(child) == depth(parent) + 1`, root depth 0.
- `I4` `name_off + name_len <= name_arena.len()` and the slice is valid UTF-8 (lossy originals
  kept in a side table).
- `I5` `NodeId` values are < store.len(); `NONE` never dereferenced.

---

## 4. `ferriscan-platform` — OS layer

```
crates/ferriscan-platform/src/
├── lib.rs                 # re-export the active implementation, shared types
├── traits.rs              # impl of core::fs traits (kept in sync with core version)
├── volumes/
│   ├── mod.rs             # Volume enumeration façade + polling helper
│   ├── windows.rs         # GetLogicalDriveStringsW / GetDiskFreeSpaceExW / labels
│   ├── linux.rs           # /proc/self/mountinfo + statvfs + pseudo-fs filtering
│   └── macos.rs           # getmntinfo + statfs
├── dir_reader/
│   ├── mod.rs
│   ├── windows.rs         # FindFirstFileExW (LARGE_FETCH) + verbatim paths
│   ├── linux.rs           # getdents64 + statx (fallback stat)
│   └── macos.rs           # getattrlistbulk (fallback readdir+lstat)
├── metadata.rs            # attribute helpers: hidden, system, reparse, compressed
├── shell/
│   ├── mod.rs             # open_path, reveal, clipboard — argv only, no shell
│   ├── windows.rs         # explorer /select, ShellExecuteW via opener
│   ├── linux.rs           # xdg-open on parent dir (best effort)
│   └── macos.rs           # open -R
├── trash.rs               # trash::delete wrapper + capability probe
├── system.rs              # locale, theme, high-contrast, reduced motion, accent color
└── tests/                 # per-platform integration tests gated by cfg
```

Rules:

- Every platform module exposes the *same* function signatures; `mod.rs` selects by `cfg`.
- All path handling uses `OsStr`/`OsString` internally; UTF-8 conversion happens at the DTO
  boundary with explicit lossy markers.
- Windows file identifiers for hardlinks require opening a handle: only done when the
  "count hardlinks" option is enabled, and failures degrade silently.
- Timeouts: no blocking call may exceed 30 s; directory reads are chunked where the API allows.

---

## 5. `ferriscan-actions` — safe mutations

```
crates/ferriscan-actions/src/
├── lib.rs
├── plan/
│   ├── mod.rs             # plan_action(): pure validation → ActionPlan
│   ├── candidates.rs      # expand node selection into filesystem items
│   ├── conflicts.rs       # destination exists, case-only, permission, free space
│   ├── protected.rs       # protected path policy per OS
│   └── token.rs           # HMAC confirm token with expiry
├── execute/
│   ├── mod.rs             # execute(): revalidate → run → report → journal
│   ├── move_.rs           # rename / EXDEV fallback / cross-volume copy→verify→delete
│   ├── copy.rs            # buffered copy, reflink attempt, temp-name strategy
│   ├── rename.rs          # case-only handling per platform
│   ├── delete.rs          # trash vs permanent, bottom-up directory removal
│   └── progress.rs        # per-item and byte progress events
├── journal/
│   ├── mod.rs             # write-ahead JSON entries, prune, list
│   └── undo.rs            # inverse replay with state verification
└── tests/                 # table-driven scenarios over real temp dirs
```

- Every operation is expressed as a sequence of `Step`s, executed sequentially; a step failure
  never aborts the whole plan silently — it records and continues unless policy says otherwise.
- TOCTOU guard: re-check `(kind, size, mtime)` captured at plan time; mismatch → `E_CHANGED`.
- No `std::process::Command` in this crate; shell actions live in `ferriscan-platform::shell`.

---

## 6. `ferriscan-persist` — local state

```
crates/ferriscan-persist/src/
├── lib.rs
├── settings.rs            # Settings struct, defaults, migration, atomic save, unknown-key keep
├── paths.rs               # per-OS dirs via `directories`, portable-mode override
├── journal_store.rs       # journal file paths + retention
├── cache.rs               # optional binary snapshot cache (off by default)
└── log.rs                 # tracing subscriber setup, rotation, redaction
```

- `Settings` is versioned (`schema: 1`), deserialized with `#[serde(default)]`, and unknown
  fields preserved through a `serde_json::Map` side-channel.
- Writes are atomic: same-directory temp file, fsync, rename, fsync directory (Unix).
- Migration functions are pure and unit-tested with fixture JSON from older versions.
- Logging redacts paths unless `verbose_logging = true`; a path hash uses BLAKE3 truncated.

---

## 7. `apps/desktop` — GPUI app (`ferriscan-app`)

```
apps/desktop/src/
├── main.rs        # window + GPUI bootstrap (windows_subsystem in release)
├── shell.rs       # Shell entity: volumes, scan, results, treemap mode, actions, settings, toasts
├── results.rs     # folder navigation over one immutable snapshot (sorted rows)
├── treemap.rs     # layout cache + hit-testing
├── theme.rs       # light/dark tokens, chart palette (high-contrast pending)
└── format.rs      # byte formatting
```

- The app is the composition root. It owns no business logic: scanning, queries, layout,
  actions, settings and strings come from the engine crates.
- Blocking work runs on `cx.background_executor()`; results return with `this.update(cx, ..)`.
  Progress is read from lock-free `Progress` counters on a 100 ms ticker.
- `Shell` is large on purpose for now; split views into `views/` and `widgets/` when a second
  contributor needs to work in parallel (the layout in ADR-0005 is the target).
- Logging uses `tracing`; the app has no `println!`.

---

## 8. UI conventions (GPUI)

- Colors, spacing and chart palette come from `theme.rs`; views take a `Theme` value.
- Lists that can exceed 200 rows use `uniform_list`; treemap and sunburst use `paint_quad` /
  paths with layout from `ferriscan-core::layout`.
- Every visible string goes through `ferriscan_i18n::Catalog` (`locales/<tag>.ftl`).
- Persisted UI state is limited to `Settings`; never node IDs or paths.
- Tests: pure view logic gets `#[cfg(test)]` unit tests; interactive paths are checked by
  launching the app (see `docs/TESTING.md`).

---

## 9. `ferriscan-cli`

```
crates/ferriscan-cli/src/
├── main.rs
├── commands/
│   ├── scan.rs       # scan PATH --json --top N --threads N
│   ├── volumes.rs    # list volumes as a table or JSON
│   └── bench.rs      # generate synthetic trees for benchmarks
└── output.rs         # human table vs JSON
```

Purpose: validate the engine before the UI, provide a CI smoke test on real filesystems, and
give contributors a fast feedback loop. Exit codes: `0` ok, `2` usage, `3` scan partial,
`4` fatal error.

---

## 10. `xtask`

Subcommands (no external task runner):

| Command | Purpose |
| --- | --- |
| `cargo xtask i18n-check` | Missing keys, placeholder mismatches, literal UI strings (not implemented yet) |
| `cargo xtask budget` | Compare latest Criterion output against `bench/budgets.toml` |
| `cargo xtask fixtures --size 1m` | Generate synthetic trees for benches/E2E |
| `cargo xtask release --target <triple>` | Build, sign hooks, checksum, changelog slice |

---

## 11. Cross-cutting conventions

### 11.1 Error handling

- Libraries: `thiserror` enums, no `anyhow` in public APIs; `anyhow` allowed in CLI/xtask only.
- The UI maps engine errors to stable `ErrorCode`s and shows the localized `error.*` message.
- `ErrorCode` is a `#[non_exhaustive]` enum in `ferriscan-core::scan::errors` and is the only
  string the UI maps to localized messages.
- Panics: `catch_unwind` at worker and command boundaries; release builds abort after logging.

### 11.2 Logging

- `tracing` spans: `scan`, `scan.dir`, `query.children`, `action.execute`.
- Fields: `node_id`, `depth`, `bytes`, `items`, `duration_ms`, `code`.
- Never log full paths at `info`; `debug`/`trace` only with verbose logging enabled.

### 11.3 Time and IDs

- Timestamps are Unix seconds (`i64`) at the model level; formatted in the UI.
- Snapshot IDs: `u64` monotonic counter + process start hash, displayed as hex.
- Plan IDs: 128-bit random hex; confirm tokens: HMAC-SHA256 truncated to 32 bytes, base64url.

### 11.4 Feature flags (Cargo)

| Feature | Crate | Purpose |
| --- | --- | --- |
| `test-fs` | core | `FakeFs`, fixture builders |
| `serde-schema` | core | emit JSON schema for DTO contract tests |
| `perf-tracing` | core | span-level counters compiled out by default |

---

## 12. Adding a feature: which files change

| Change | Files |
| --- | --- |
| New scan option | `core/scan/mod.rs` (options), `apps/desktop/src/shell.rs` (options wiring), `persist/settings.rs` if persisted, locale file |
| New view | `core/query/*` or `core/layout/*` (if new data shape), a view in `apps/desktop/src`, locale keys, unit tests |
| New file action | `actions/plan/*`, `actions/execute/*`, `platform` shell call if OS-specific, action bar in `shell.rs`, tests in `actions/tests` |
| New platform quirk | `ferriscan-platform/<area>/<os>.rs` + `docs/PLATFORM_MATRIX.md` |
| New locale | `crates/ferriscan-i18n/locales/<tag>.ftl`, `docs/SPEC.md` §14 |
| New setting | `persist/settings.rs`, settings panel in `shell.rs`, locale keys, migration test |

If a change touches more than six of these areas, it needs an ADR first.
