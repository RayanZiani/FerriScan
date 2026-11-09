# FerriScan — Product & Technical Specification

| | |
| --- | --- |
| **Status** | Draft, authoritative until superseded by an ADR |
| **Version** | 0.1.0 |
| **Last updated** | 2026-11-04 |
| **Owners** | Product (spec), Core (engine), UI (interface) |
| **License** | MIT |

This document is the single source of truth for **what FerriScan is**, **how it is built**, and
**how we know it works**. Anything not described here is out of scope for v1.0.

---

## Table of contents

1. [Product definition](#1-product-definition)
2. [Personas and jobs](#2-personas-and-jobs)
3. [Scope](#3-scope)
4. [Functional requirements](#4-functional-requirements)
5. [Non-functional requirements](#5-non-functional-requirements)
6. [Performance model and budgets](#6-performance-model-and-budgets)
7. [Architecture](#7-architecture)
8. [Scanning engine](#8-scanning-engine)
9. [Data model](#9-data-model)
10. [Engine API](#10-engine-api)
11. [Visualization](#11-visualization)
12. [File operations and safety](#12-file-operations-and-safety)
13. [Settings, persistence and privacy](#13-settings-persistence-and-privacy)
14. [Internationalization](#14-internationalization)
15. [Theming and accessibility](#15-theming-and-accessibility)
16. [Security model](#16-security-model)
17. [Testing and quality gates](#17-testing-and-quality-gates)
18. [Packaging, updates and distribution](#18-packaging-updates-and-distribution)
19. [Risks and mitigations](#19-risks-and-mitigations)
20. [Glossary](#20-glossary)

---

## 1. Product definition

**FerriScan** is a native-feeling desktop application that measures and explains disk usage.
It scans a volume or a folder, builds an in-memory index of everything it found, and lets the
user explore that index through several complementary visualizations, then act on the files
(move, copy, rename, trash, delete) with explicit confirmation and undo where possible.

The name comes from *ferrite*: a material that stores information densely and reads it fast.
That is the engineering thesis of the product — a dense, cache-friendly index behind a fast,
calm interface.

**One-sentence pitch:** FerriScan tells you exactly where your disk space went, in seconds,
without sending a single byte anywhere.

**Differentiators**

1. **Speed as a feature, measured.** Performance budgets are enforced by benchmarks in CI.
2. **Radical privacy.** No account, no telemetry, no network code path in the scanning or
   analyzing path.
3. **Actions, not just charts.** The analyzer is also a safe file manager for the results:
   plan → confirm → execute → undo (moves) / report (deletes).
4. **Craft.** Native theme integration, 12 languages, keyboard-complete, WCAG 2.2 AA.

**Anti-goals**

- Cloud storage integration, accounts, sync, sharing.
- Real-time background monitoring daemon (v2 candidate, not v1).
- Duplicate-file detection by content hashing (v1.1 candidate).
- Network/remote scanning over SSH/SMB enumeration (v2 candidate).
- Mobile or web builds.

---

## 2. Personas and jobs

| Persona | Context | Job to be done |
| --- | --- | --- |
| **Cleaner** | Home user, 256 GB laptop at 97% | "Show me the biggest things I can safely remove, then remove them." |
| **Developer** | Node/target/Rust caches, Docker images, VMs | "Which project folders are eating 40 GB, and what is regenerable?" |
| **IT supporter** | Repairing someone else's machine | "Scan this drive without installing anything invasive; find the hog; move it to an external disk." |
| **Archivist** | Media libraries on external drives | "Where did my space go, by folder and by file type, across this 4 TB disk?" |

**Top user stories** (numbered for traceability in tests):

- `US-01` As a user, I see all mounted volumes with capacity, free space, filesystem and type.
- `US-02` As a user, I can scan a whole volume or a folder, and cancel at any time.
- `US-03` As a user, I see live progress (items, bytes, current path, elapsed, rate) while scanning.
- `US-04` As a user, I explore results as a tree table, treemap, sunburst, extension chart and
  largest-files list, and switch views without rescanning.
- `US-05` As a user, I can sort by size, allocated size, item count, name, type or modification
  date, ascending or descending, in every view that supports ordering.
- `US-06` As a user, I can filter by name substring, extension, size range and age.
- `US-07` As a user, I can search the current snapshot by name and jump to a result.
- `US-08` As a user, I can select one or many items and move, copy, rename, trash or permanently
  delete them after reviewing a plan that states exactly what will happen.
- `US-09` As a user, I can undo a move I just performed.
- `US-10` As a user, I can open an item or reveal it in the OS file manager.
- `US-11` As a user, the app follows my OS light/dark theme and language, and I can override both.
- `US-12` As a user, my preferences survive restarts without any account and nothing leaves my machine.
- `US-13` As a user, I can export a scan report (CSV/JSON) of the current view.
- `US-14` As a user with a screen reader or keyboard only, I can complete every task above.

---

## 3. Scope

### 3.1 In scope for v1.0

- Windows 10 1809+ / Windows 11 (x64, arm64) — **primary**.
- Linux (x64, arm64) — GTK/WebKitGTK, AppImage + `.deb` + `.rpm`.
- macOS 11+ (arm64, x64) — `.dmg`, signed and notarized if a certificate is available.
- Local fixed, removable and (read-only) network volumes exposed by the OS.
- Full-featured analyzer + safe file actions as described above.

### 3.2 Out of scope for v1.0

Scheduled scans, watchers, duplicate detection, archive inspection (zip contents), cloud
providers, MFT/USN direct journal parsing (evaluated for v1.2), elevated helper service,
CLI daemon, plugin system, custom chart scripting.

### 3.3 Explicit platform parity rules

Anything that exists on one platform must exist on the others or be hidden with a documented
reason (`docs/PLATFORM_MATRIX.md`). "Reveal in file manager" and "trash" are the only features
allowed to differ in *implementation*, never in *presence*.

---

## 4. Functional requirements

Requirement IDs are stable and referenced by tests and PRs.

### FR-1 Volumes

- `FR-1.1` Enumerate mounted volumes at startup and on demand (refresh button + 5 s polling
  while the volume list is visible, stopped when a scan runs).
- `FR-1.2` For each volume report: mount point, display label, filesystem, kind
  (`fixed|removable|network|optical|ram|unknown`), total bytes, available bytes, read-only flag,
  and whether the user can write.
- `FR-1.3` Windows: use `GetLogicalDriveStringsW`, `GetDriveTypeW`, `GetVolumeInformationW`,
  `GetDiskFreeSpaceExW`. Show BitLocker-locked volumes as locked instead of failing silently.
- `FR-1.4` Linux: parse `/proc/self/mountinfo`, filter pseudo filesystems
  (`proc`, `sysfs`, `cgroup2`, `tmpfs` optional, `overlay` under containers), use `statvfs`.
- `FR-1.5` macOS: `getmntinfo` + `statfs`, skip `devfs`, `autofs` unless mounted.
- `FR-1.6` A volume that disappears mid-scan produces a non-fatal warning and a partial snapshot.
- `FR-1.7` Clicking a volume starts a scan of its mount point with default options.

### FR-2 Scan lifecycle

- `FR-2.1` States: `idle → preparing → scanning → finalizing → ready`, plus `cancelling`,
  `cancelled`, `failed`, `partial`. Only one scan at a time per window (v1).
- `FR-2.2` The UI stays responsive at all times; scanning runs on a dedicated thread pool and
  never holds the UI thread or a lock longer than a batch.
- `FR-2.3` Progress events at most 10 Hz, coalesced: items seen, bytes seen, current directory,
  errors count, elapsed, items/s, bytes/s.
- `FR-2.4` Cancel is immediate (≤ 250 ms to stop scheduling new work) and keeps what was already
  indexed, marked `cancelled` with partial results usable.
- `FR-2.5` Re-scan while a scan runs cancels and restarts (explicit UI confirmation).
- `FR-2.6` Scan options: follow symlinks (default **off**), cross filesystem boundaries
  (default **on** for folder scans, irrelevant for volume scans), include hidden/system files
  (default on), exclude globs (default list below), minimum file size (default 0), max depth
  (default unlimited), threads (default auto).
- `FR-2.7` Default excludes: `$RECYCLE.BIN`, `System Volume Information`, `/proc`, `/sys`,
  `/dev`, `/run`, `/.Spotlight-V100`, `/.fseventsd`, `/.Trashes`.
- `FR-2.8` Errors (permission denied, path too long, vanished entry, I/O) are recorded, counted,
  and browsable in an "Issues" panel; they never abort the scan.
- `FR-2.9` Windows long paths: all filesystem calls use `\\?\` verbatim prefixes internally.
- `FR-2.10` Scan results are immutable; a new scan creates a new snapshot.

### FR-3 Exploration

- `FR-3.1` Tree table: columns Name, Size, Allocated, Items, % of parent, Modified, Type.
  Sorting by any column, ascending/descending, persisted per view.
- `FR-3.2` Children are loaded lazily, paginated (default 500 rows, virtualized rendering).
- `FR-3.3` Breadcrumb path navigation reflecting the selected node; click any crumb to navigate.
- `FR-3.4` Treemap: squarified layout, depth-limited, with zoom into a node, hover tooltip,
  click to select, double-click to make it the new root, and a "back to parent" affordance.
- `FR-3.5` Sunburst: hierarchical ring chart with the same zoom/select semantics; center shows the
  current node name and size.
- `FR-3.6` Extension view: top N extensions by total size with file count, color-coded and
  clickable to filter.
- `FR-3.7` Largest files: global top 100 (configurable 10–1000), respecting active filters.
- `FR-3.8` Filters compose: name substring (case/accent-insensitive), extension set, size range,
  modified-age range, and "kind" (files/folders/all).
- `FR-3.9` Search is instant over the current snapshot: prefix/substring match, result count
  capped at 1000, each hit shows a full path built by walking parents.
- `FR-3.10` Every view keeps selection in sync (selecting in treemap selects in the table).
- `FR-3.11` Empty, loading, error and permission-denied states each have a designed component.

### FR-4 Actions

- `FR-4.1` Supported: Open, Reveal in file manager, Copy path, Move to…, Copy to…, Rename,
  Delete to Trash, Delete permanently.
- `FR-4.2` Every mutating action runs through a **plan** (see §12) shown to the user before
  execution, listing each source, destination, conflict status and total bytes.
- `FR-4.3` Trash is the default delete. Permanent delete requires an explicit choice plus typing
  `DELETE` in a confirmation field for more than one item or more than 1 GiB.
- `FR-4.4` Cross-volume moves are copy-then-verify-then-delete, never a partial rename.
- `FR-4.5` Moves and copies are journalled and can be rolled back for 24 h (last 10 operations).
- `FR-4.6` Conflicts (existing destination, case-only rename, permission) are surfaced per item
  with `Skip / Replace / Keep both / Cancel all` choices.
- `FR-4.7` After any mutation the affected subtree is patched in the snapshot (sizes recomputed
  up the parent chain and the filesystem re-read for the touched directory), with a visible
  "results updated" indicator. No full rescan.
- `FR-4.8` System-critical paths are blocked by default (`C:\Windows`, `C:\Program Files*`,
  `/System`, `/usr`, `/bin`, `/etc`, `/boot`, the app's own directory) with an advanced
  override that requires a second confirmation and is never remembered.
- `FR-4.9` Open uses the OS association; Reveal uses Explorer / `xdg-open` on the parent /
  `open -R`. No shell interpolation: arguments are passed as argv, never as a command string.

### FR-5 Settings

- `FR-5.1` Language: follow system or explicit choice from 12 locales.
- `FR-5.2` Theme: system / light / dark; accent follows the OS unless overridden.
- `FR-5.3` Density: comfortable / compact.
- `FR-5.4` Default view, default sort, scan options, exclude list, confirmations, units
  (binary/decimal), show allocated size, animation toggle, telemetry (absent — no toggle).
- `FR-5.5` All settings apply immediately and persist debounced (300 ms) to a versioned JSON file.
- `FR-5.6` "Reset all settings" restores defaults after confirmation.
- `FR-5.7` Settings survive app updates; unknown keys are preserved on write (forward compatibility).

---

## 5. Non-functional requirements

| ID | Requirement |
| --- | --- |
| `NFR-1` | Cold start to interactive volumes list ≤ 600 ms on a mid-range machine (debug builds excluded). |
| `NFR-2` | UI never blocks > 16 ms per frame during scanning; no frame drop > 100 ms while idle. |
| `NFR-3` | Memory ≤ 96 bytes per indexed entry, excluding name bytes (target ~80). |
| `NFR-4` | 1,000,000 entries scanned in ≤ 12 s on warm NVMe, ≤ 45 s on cold NVMe. |
| `NFR-5` | Children page of 500 rows served in ≤ 10 ms P95 for a directory with 200k children. |
| `NFR-6` | Treemap of 5,000 nodes laid out in ≤ 25 ms in Rust. |
| `NFR-7` | Installer ≤ 15 MB per platform; installed footprint ≤ 40 MB. |
| `NFR-8` | Idle memory ≤ 120 MB with a 1M-entry snapshot loaded. |
| `NFR-9` | No network syscalls except the opt-in update check (disabled in v1.0 builds). |
| `NFR-10` | No crash on unreadable, cyclic, deep (≤ 4,096) or non-UTF-8 paths. |
| `NFR-11` | All user-visible strings localized; zero hard-coded UI text. |
| `NFR-12` | WCAG 2.2 AA: contrast, focus visible, keyboard reachability, reduced motion, screen reader names. |
| `NFR-13` | Reproducible release builds; `cargo deny` clean; no GPL/AGPL dependencies. |
| `NFR-14` | Unsigned-script-free operations: no `sh -c`, no `cmd /c`, no `powershell -Command` with interpolation. |

---

## 6. Performance model and budgets

### 6.1 Where time goes

A scan is dominated by **metadata syscalls**, not by CPU. Therefore:

1. Read directories with the platform's bulk API (one syscall returns many entries).
2. Avoid a second `stat` per entry whenever the enumeration already returns size/time.
3. Keep the hot loop allocation-free; names go into a preallocated byte arena.
4. Parallelize at the *directory* level with a work-stealing queue; files are processed inline
   by the thread that owns the directory.
5. Keep the node store compact and append-only during the scan; never sort during the scan.
6. Derive aggregates (folder sizes, counts) in a single post-order pass at `finalizing`.

### 6.2 Threading policy

- Threads = `clamp(available_parallelism, 2, 16)` by default; user can cap it.
- Each worker owns a local `Vec<DirJob>` LIFO stack for locality; overflow goes to a global
  lock-free deque.
- A semaphore limits in-flight directory reads to `4 × threads` to bound memory.
- Progress counters are `AtomicU64` relaxed; the reporter thread reads them at 10 Hz.
- On spinning disks, an adaptive controller reduces concurrency when the measured
  seeks/second proxy (items/s variance) degrades; detected via `kind == fixed` and a heuristic,
  overridable in advanced settings.

### 6.3 Budgets enforced in CI (`cargo bench` + thresholds in `bench/budgets.toml`)

| Benchmark | Budget |
| --- | --- |
| `scan_synthetic_1m` (warm page cache) | ≤ 12 s, ≥ 90k entries/s |
| `scan_synthetic_1m` peak RSS | ≤ 110 MB |
| `children_page_200k` | ≤ 10 ms P95 |
| `treemap_layout_5k` | ≤ 25 ms P95 |
| `sunburst_layout_5k` | ≤ 10 ms P95 |
| `snapshot_aggregate_1m` | ≤ 250 ms |
| `search_1m_substring` | ≤ 80 ms P95 |

A benchmark exceeding its budget by > 15 % fails PR CI; > 5 % posts a warning. Budgets live in
version control and may only change with a documented ADR.

---

## 7. Architecture

### 7.1 Stack

| Layer | Technology | Rationale |
| --- | --- | --- |
| Shell + UI | **GPUI** (pinned Zed git revision), 100 % Rust, no webview (ADR-0005) | One language, direct access to `Arc<Snapshot>`, GPU rendering, no IPC. |
| Charts | GPUI canvas renderers (treemap, sunburst) fed by `core::layout` arrays | Predictable frame budget at 5k+ nodes. |
| Engine | Rust 2024 edition, one workspace | Direct OS APIs, zero-copy index, deterministic performance. |
| IPC | None: the UI calls the engine crates directly | No serialization boundary to budget or secure. |
| i18n | Fluent (`.ftl`) via `ferriscan-i18n`; stable error codes from Rust | Plural/gender correctness, no string concatenation. |
| Storage | Plain versioned JSON in the OS config dir | Auditable, portable, no database dependency. |
| Logging | `tracing` to a rotating local file, off by default at `info` | Diagnosable without telemetry. |

See `docs/adr/0005-gpui-native-ui.md` for the choice of GPUI and its known risks, and
`docs/adr/0001-tauri-svelte.md` (superseded) for the alternatives first evaluated (Electron,
egui, iced, Qt, Slint).

### 7.2 Process and data flow

```
                 ┌──────────────────────────── Windows / Linux / macOS ───────────────────────────┐
                 │                                                                                 │
  ┌──────────────▼──────────────┐        direct calls (Arc<Snapshot>)         ┌──────────────────┐  │
  │  GPUI app (ferriscan-app)   │ ◄────────────────────────────────────────► │  engine crates   │  │
  │  views, theme, painters     │   background tasks, progress counters      │  (same process)  │  │
  └─────────────────────────────┘                                            └───────┬──────────┘  │
                                                                                     │             │
                                          ┌──────────────────────────────────────────┼─────────────┘
                                          │                                          │
                                 ┌────────▼────────┐                        ┌────────▼─────────┐
                                 │ ferriscan-core  │                        │ ferriscan-actions│
                                 │ scan, index,    │                        │ plan, execute,   │
                                 │ aggregate,      │                        │ journal, undo    │
                                 │ layout, search  │                        └────────┬─────────┘
                                 └────────┬────────┘                                 │
                                          │                                          │
                        ┌─────────────────┴───────────────┐                ┌─────────▼─────────┐
                        │ ferriscan-platform              │                │ ferriscan-persist │
                        │ volumes, dir enumeration,       │                │ settings, cache,  │
                        │ metadata, trash, reveal, theme  │                │ journal storage   │
                        └─────────────────────────────────┘                └───────────────────┘
```

Rules:

- `ferriscan-core` has **no** UI dependency, no OS-specific `cfg` beyond a narrow platform trait, and no
  filesystem mutation. It is testable on any machine with a synthetic filesystem trait.
- `ferriscan-platform` owns every `unsafe` OS call and every `cfg(target_os)`.
- `ferriscan-actions` is the only crate allowed to delete/move/copy user data.
- The app crate (`ferriscan-app`) is the composition root: views, theme, state, background tasks.
- Views never walk the whole tree per frame; they use paged queries, aggregates and cached
  layout rectangles.

### 7.3 State ownership

- `AppState` (Rust): `Arc<AppStateInner>` holding `RwLock<Option<Arc<Snapshot>>>`, the scan
  controller (`Mutex<ScanController>`), settings (`ArcSwap<Settings>`), and an action journal.
- `Snapshot` is immutable and `Arc`-shared; patching after a mutation creates a new snapshot
  (structural sharing of untouched subtrees is *not* attempted in v1; a patch clones the node
  vectors, acceptable because it happens at human pace and is chunked).
- UI state lives in the `Shell` entity: scan state, results (snapshot, current folder, rows),
  selection, view mode, settings, toasts. Only settings are persisted (never paths).

---

## 8. Scanning engine

### 8.1 Pipeline

```
prepare → enumerate (parallel, streaming) → finalize (aggregate) → publish snapshot
```

1. **Prepare**: canonicalize root, validate options, build the exclude matcher (globset),
   allocate arenas with capacity hints (4M nodes / 64 MB name arena by default, grown
   geometrically), reset counters, set `generation = now`.
2. **Enumerate**: jobs are `(dir_id, path)`; a worker opens the directory with the platform
   enumerator, appends children to the local buffer, links them into the store under a short
   `Mutex<StoreWriter>` critical section (batched, ~512 nodes per lock), pushes subdirectories
   onto its stack. Files are recorded immediately. Directories get a placeholder node and are
   revisited to fill their size after aggregation.
3. **Finalize**: post-order traversal computing `size = files + children`, `item_count`,
   `max_depth`, extension totals, `largest_files` heap (top 1000), and the sorted child order
   for the default view. Runs in parallel over top-level subtrees, merged deterministically.
4. **Publish**: wrap in `Arc<Snapshot>`, store in `AppState`, emit `scan://done` with metadata.

### 8.2 Platform enumerators

| Platform | API | Notes |
| --- | --- | --- |
| Windows | `FindFirstFileExW(FindExInfoBasic, FIND_FIRST_EX_LARGE_FETCH)` + `FindNextFileW` | Returns `WIN32_FIND_DATAW`: name, attributes, size, times. Directories get a second enumeration pass lazily only if needed; no `GetFileAttributes` per entry. |
| Linux | `getdents64` via `nix`/`libc`, then `statx(AT_STATX_DONT_SYNC, STATX_SIZE|STATX_MTIME|STATX_MODE|STATX_INO)` | `statx` is one syscall; batching is not available, so concurrency matters. Fallback to `stat` when `statx` is unavailable (kernel < 4.11). |
| macOS | `getattrlistbulk` with `ATTR_CMN_NAME|ATTR_CMN_RETURNED_ATTRS|ATTR_CMN_OBJTYPE|ATTR_CMN_MODTIME|ATTR_CMN_FILEID|ATTR_CMN_DEVID` + `ATTR_FILE_DATALENGTH|ATTR_FILE_ALLOCSIZE|ATTR_DIR_ENTRYCOUNT` | Returns many entries per syscall; falls back to `readdir` + `lstat` on failure. |

A `DirReader` trait keeps the core generic; an in-memory `FakeFs` implements it for tests and
benchmarks so results are reproducible.

### 8.3 Error taxonomy

Every failure maps to a stable code (`E_PERM`, `E_NOTFOUND`, `E_TOOLONG`, `E_LOOP`,
`E_IO`, `E_INVALID`, `E_BUSY`, `E_CANCELLED`, `E_UNSUPPORTED`) with structured context
(path hash, operation, OS error number). Codes are localized through `ferriscan-i18n`; raw OS messages
are shown in an expandable "technical details" block.

### 8.4 Cancellation and lifecycle

- `ScanController` holds `Arc<AtomicBool>` and a generation counter.
- Workers check the flag before each directory; an in-progress directory read is allowed to
  finish (bounded by one syscall).
- Cancel path: set flag → join with 250 ms grace → if still running, mark result `cancelled`
  and publish whatever is indexed (the writer lock guarantees a consistent store).
- Dropping the app cancels and joins; no detached threads.
- `panic = "abort"` in release; a panic during scan is caught at the worker boundary with
  `catch_unwind` and converted into `E_IO` for that subtree so one poisoned directory cannot
  kill the app.

### 8.5 Hardlinks, symlinks, sparse and compressed files

- **Symlinks**: not followed. Recorded as a leaf with `kind = symlink`, size 0, `link_size` =
  length of target string. Following requires the opt-in option; loop detection uses an
  ancestor set of `(dev, ino)` / volume file id with a cap of 1,024 entries.
- **Hardlinks**: Windows and Unix record `(dev, file_id)`. With "count hardlinks once" enabled
  (default on Unix, off on Windows because the ID requires an extra handle open), the second
  occurrence is recorded with `shared = true` and size 0, and reported in a summary.
- **Sparse files**: report both logical (`size`) and on-disk (`allocated`) sizes; the UI shows
  allocated by default with a toggle.
- **Compressed/encrypted (NTFS)**: `allocated` reflects the compressed size; a badge marks
  compressed/encrypted attributes.

---

## 9. Data model

### 9.1 Node store (SoA, cache friendly)

Flat array of 56-byte records; parent/child/sibling links instead of nested structures. No
paths stored — a path is reconstructed by walking `parent` and reading names from the arena.

```rust
#[repr(C)]
pub struct NodeRecord {
    name_off: u32,      // offset into the name arena
    name_len: u16,      // UTF-8 bytes, max 65_535
    ext_id: u16,        // index into the extension dictionary (0 = none)
    parent: u32,        // NodeId::NONE for root
    first_child: u32,   // first child or NONE
    next_sibling: u32,  // next sibling or NONE
    child_count: u32,
    size: u64,          // logical bytes (files only, folders after aggregation)
    allocated: u64,     // on-disk bytes
    mtime: i64,         // Unix seconds, 0 = unknown
    depth: u16,
    flags: u16,         // dir|file|symlink|hidden|system|readonly|shared|compressed|encrypted|error
}
```

- `NodeId` is a `u32` newtype; `NONE = u32::MAX`. Capacity limit 4.29 B nodes, far above any
  real volume.
- Names: 16 MB default arena, grown by doubling; `&str` views are validated once at insertion
  (lossy replacement only for display, original bytes retained for filesystem operations).
- Extension dictionary: interned lowercase extensions → `Vec<String>`, plus per-extension
  totals computed at finalize.
- Children of a directory are reachable through the sibling list; for directories with more than
  8k children an optional sorted index is built at finalize keyed by the active sort.
- The store exposes **no** interior mutation after publish; patch operations produce a new
  `NodeStore` by cloning and applying a delta list.

```rust
pub struct Snapshot {
    pub id: SnapshotId,             // ULID-ish, monotonic
    pub root: NodeId,
    pub store: NodeStore,
    pub ext_index: ExtensionIndex,
    pub meta: ScanMeta,             // root path, volume, options, timings, counts, errors summary
    pub largest: Vec<NodeId>,       // top 1000 by size
    pub status: SnapshotStatus,     // Complete | Partial | Cancelled
}
```

### 9.2 Derived aggregates

Computed once at finalize and cached in the snapshot:

- per-node `size`, `allocated`, `item_count`, `file_count`, `folder_count`
- per-extension `{files, bytes}` totals (global and per top-level subtree)
- top-N largest files, top-N largest folders
- depth histogram and age histogram (for the overview cards)

### 9.3 DTOs at the boundary

DTOs shared with the CLI and persistence are `serde` structs with `camelCase` renaming, integer byte sizes, string enums, and
explicit `null` for unknown values. Node identity is `u32`; no DTO ever contains the full tree.
See §10 for the engine API the UI calls.

---

## 10. Engine API

There is no IPC. `ferriscan-app` calls the engine crates in-process; blocking work runs on the
GPUI background executor and results return to the view with `this.update`. Public engine
functions keep stable names; changing a signature needs a note in the PR and tests.

| UI need | Engine call |
| --- | --- |
| List volumes | `ferriscan_platform::list_volumes` |
| Scan | `ferriscan_core::scan::scan` with `Progress` and `CancelToken` (polled at 10 Hz) |
| Children page / breadcrumb | `ferriscan_core::query::children_page`, `NodeStore::relative_path` |
| Treemap / sunburst | `ferriscan_core::layout::{treemap, sunburst}` (flat tile/arc arrays) |
| Search | `ferriscan_core::query::search` |
| Plan, execute | `ferriscan_actions::plan`, `ferriscan_actions::execute::execute` |
| Open, show in folder, trash | `ferriscan_platform::shell::{open_path, trash}` (via actions for trash) |
| Settings | `ferriscan_persist::Settings::{load, save}` |
| Strings | `ferriscan_i18n::Catalog` |

Rules:

- The UI keeps node IDs and resolves paths from the snapshot at the point of use.
- Every mutation goes through `ferriscan-actions` (plan, review, execute). ADR-0003 designed an
  HMAC confirm token against a compromised webview; the native UI runs plan then execute
  in-process without that token today. Whether to keep, adapt or retire the token is an open
  item for `security-reviewer` (tracked in ADR-0005 follow-ups), not a decision made here.
- Errors carry stable `ErrorCode`s mapped to `error.*` catalog messages.
- DTO page caps (≤ 500 rows) still apply to `children_page`; the table builds its own full
  sorted list for virtualized display.

---

## 11. Visualization

### 11.1 Treemap (primary)

- **Algorithm**: squarified (Bruls et al.), computed in Rust over the node subtree, depth-limited
  (default 8) and budget-limited (default 5,000 rectangles). Remaining children are aggregated
  into a `+ N more` cell that is not clickable but shows tooltip totals.
- **Rendering**: GPUI `paint_quad`, layout cached per folder and size; hover uses the cached rectangles (a spatial hash is the upgrade path)
  rather than per-rect DOM nodes. Text labels only when the rect exceeds measured thresholds.
- **Color**: by extension family (categorical palette, 12 hues, lightness ramps by depth),
  or by age / size / type via a legend toggle. Palette is color-blind safe and contrast-checked.
- **Interaction**: click selects, double-click re-roots, wheel/⌘+wheel zooms, `Backspace` up,
  hover tooltip with name, size, % of root, item count.
- **Empty/degenerate**: single-file roots, zero-byte folders and 1 px rects are handled without
  infinite loops.

### 11.2 Sunburst (secondary)

- Two-level minimum; arc angles computed in Rust, painted with a hover highlight.
- Same zoom/select semantics as the treemap; center label shows current root.

### 11.3 Tree table (authoritative)

- Virtualized rows (TanStack Virtual or a custom 60-line virtualizer), fixed row height,
  sticky header, resizable columns persisted globally.
- Indentation by depth with expand/collapse carets; expansion state is ephemeral (per session).
- Multi-select with Shift/Ctrl, `Space` to toggle, `Enter` to open, `Delete` to plan deletion.
- Inline size bars behind the "Size" cell (subtle, off in high-contrast mode).

### 11.4 Extension and largest-files views

- Horizontal bars with animated width (disabled with reduced motion), sorted desc, top 20 by
  default with "show all".
- Largest files as a compact table with path, size, type, modified date and quick actions.

### 11.5 Overview cards

After a scan: total size, files, folders, largest folder, largest file, error count, scan
duration, and a one-line "top 3 hogs" summary. Cards are clickable shortcuts into filtered views.

---

## 12. File operations and safety

This is the highest-risk area of the product. It is specified as a state machine.

### 12.1 Plan → confirm → execute → report

1. **Plan** (`plan_action`) is pure: it validates inputs, resolves canonical paths, checks
   existence, free space, conflicts, permissions, protected paths, and returns:
   - `planId` (random 128-bit), `confirmToken` (HMAC of plan id + serialized plan + app secret),
     `expiresAt` (5 min), `items[]` with `{ source, destination, action, status, reason? }`,
     `summary` (count, bytes, crossVolume, needsElevation, blocked).
2. **Confirm** in the UI: the user sees the summary, must tick an acknowledgement for
   destructive plans, and types `DELETE` when the plan is permanent and large/multi-item.
3. **Execute** (`execute_action`): the backend re-validates the token and the plan's expiry,
   re-checks each item (TOCTOU guard), and runs it with progress events. Items are processed
   sequentially for determinism; large trees are reported per top-level item.
4. **Report**: `{ succeeded, failed, skipped, bytesMoved, journalId? }`; failures keep their
   error codes and per-item reason. The UI offers "Retry failed" (new plan) and "Undo".

### 12.2 Operation semantics

| Operation | Same volume | Cross volume | Failure policy |
| --- | --- | --- | --- |
| Move | atomic rename; if `EXDEV`, fall back to copy+verify+delete | copy to temp name in destination dir, fsync, rename into place, verify size+mtime, delete source | Never leave a half-written destination under the final name |
| Copy | stream with 1 MiB buffer, preserve mtime, optional `reflink` where supported | same | Partial temp file removed |
| Rename | `rename` / `MoveFileExW` | n/a | Case-only differences handled per platform |
| Trash | `trash` crate → OS recycle bin / FreeDesktop trash | n/a | If trash is unavailable (network share), offer permanent delete explicitly |
| Permanent delete | files unlinked; directories bottom-up with depth limit and symlink safety | n/a | Continue on error, aggregate failures |

- **Symlink safety**: operations act on the link itself, never the target, unless the user
  selected "follow" in the plan.
- **Long paths**: `\\?\` prefix; Windows destination paths are validated for invalid characters.
- **Open handles**: on Windows, `ERROR_SHARING_VIOLATION` is reported as `E_BUSY` with a retry
  option; no forced close, no `MoveFileEx` with delay-until-reboot.
- **Free space**: cross-volume moves require `destination_free >= plan_bytes × 1.02`; the UI
  shows the shortfall.

### 12.3 Protected paths

Blocked by default, with a non-persisted override flow:

- Windows: `%SystemRoot%`, `%ProgramFiles%`, `%ProgramFiles(x86)%`, `%ProgramData%\Microsoft`,
  the app's install dir, any path with the `system` attribute at the volume root.
- Linux: `/`, `/boot`, `/etc`, `/usr`, `/bin`, `/sbin`, `/lib*`, `/proc`, `/sys`, `/dev`, `/run`,
  and the user's home *root itself* (children are fine).
- macOS: `/System`, `/Library`, `/usr`, `/bin`, `/sbin`, `/private`, `/Applications` (with
  per-item override for the latter).

Blocking is evaluated on canonical, case-normalized paths with symlink resolution of the parent.

### 12.4 Journal and undo

- Every move/copy writes a journal entry *before* touching data (write-ahead) in
  `<app data>/journal/<timestamp>-<id>.json`, fsynced.
- Undo replays the inverse in reverse order, verifying that sources/destinations are still in
  the expected state; otherwise it refuses and explains.
- Journals are pruned after 30 days or when more than 50 exist; permanent deletes get a
  non-undoable record for transparency.
- Journal files contain paths (user data). They never leave the machine and can be wiped from
  Settings → Privacy → "Clear activity history".

---

## 13. Settings, persistence and privacy

- **Location (per OS)**:
  - Windows: `%APPDATA%\FerriScan\settings.json`
  - Linux: `$XDG_CONFIG_HOME/ferriscan/settings.json` (fallback `~/.config`)
  - macOS: `~/Library/Application Support/FerriScan/settings.json`
- **Format**: versioned JSON, atomic write (temp + rename), preserving unknown keys.
- **Persisted**: theme, language, density, units, default view/sort, column widths, scan
  options, exclude list, confirmation preferences, window geometry, last scanned volume *label*
  (not path) for the "recent" list — opt-out available.
- **Never persisted**: file listings, scanned paths, search queries, file contents, environment
  variables, usernames.
- **In-memory only**: snapshots, selections, expansion state, current filters (except the
  defaults above).
- **Optional cache** (off by default): a binary snapshot cache keyed by volume ID + root + mtime,
  stored in the OS *cache* dir, deleted on app exit unless "keep cache" is enabled.
- **Logs**: local rotating file, `info` level, no paths by default (`paths` redacted to hashes
  unless the user enables verbose logging).
- **No telemetry, no crash reporting upload, no analytics, no update ping in v1.0.**
  See `docs/PRIVACY.md` for the full data inventory.

---

## 14. Internationalization

- **Locales v1.0**: `en`, `fr`, `de`, `es`, `it`, `pt-BR`, `nl`, `pl`, `ru`, `tr`, `ja`, `zh-CN`.
- **Format**: Fluent `.ftl` files in `apps/desktop/src/lib/i18n/locales/<tag>/`. One file per
  feature area; keys are semantic (`scan.progress.items`, `actions.delete.confirm_title`).
- **Selection**: `sys-locale` on the Rust side at startup; the user override wins. Fallback
  chain: exact tag → language → `en`. RTL is architected for (logical CSS properties) but not
  shipped in v1.0.
- **Numbers and dates**: `Intl.NumberFormat`/`Intl.DateTimeFormat`; byte units localized
  (`1.4 GB` vs `1,4 Go`). Binary vs decimal units is a setting.
- **Plurals and gender**: always through Fluent selectors; never string concatenation.
- **Raw OS errors**: shown as localized code + expandable original message.
- **CI check**: `cargo xtask i18n-check` fails on missing keys, unknown placeholders, or
  untranslated strings in a shipped locale.

---

## 15. Theming and accessibility

### 15.1 Theme

- Tokens only (`--fs-bg`, `--fs-surface`, `--fs-text`, `--fs-muted`, `--fs-accent`,
  `--fs-danger`, `--fs-border`, `--fs-focus`, semantic chart palette). No hard-coded colors in
  components.
- `prefers-color-scheme` drives system mode; the Rust side also pushes `system://theme` when the
  OS theme changes, and mirrors it to the native window (`theme` API) so title bars match.
- Windows accent color read from the registry (`DwmGetColorizationColor` fallback) for the
  primary action and focus ring; contrast-checked against the surface token and darkened/
  lightened automatically if it fails AA.
- High-contrast mode: borders become 1 px opaque, chart patterns replace color-only encoding,
  size bars disabled.
- Reduced motion: no transitions over 100 ms, no chart animations.

### 15.2 Accessibility

- Full keyboard model: `Tab` order, arrow navigation in trees and treemaps (grid semantics),
  `Enter` activate, `Space` select, `F2` rename, `Delete` plan delete, `Ctrl+F` search,
  `Ctrl+A` select all in view, `Esc` close overlay / cancel scan.
- Every interactive element has an accessible name; charts expose a parallel table view via
  "Accessible data" toggle (screen-reader users get real `<table>` markup).
- Focus is visible (3:1 against adjacent colors) and never trapped.
- Announcements via an `aria-live="polite"` region for scan start/end, action results, errors.
- Accessibility goes through GPUI's AccessKit integration, which is not yet verified here (see
  ADR-0005 risks). Manual screen-reader pass per release (Narrator/NVDA on Windows, Orca on
  Linux, VoiceOver on macOS) until automated checks exist.

---

## 16. Security model

- **No embedded browser**: the UI is native GPUI, so there is no CSP, remote content or
  capability allowlist to maintain.
- **Path handling**: the UI keeps *node IDs*; paths are resolved from the snapshot when an
  action runs. Paths from outside (export destination) come from a native dialog.
- **Command execution**: never a shell. `Command::new(program).args([...])` only, with a
  per-platform constant program list.
- **Supply chain**: `cargo deny` (advisories, licenses, sources), `cargo audit` in CI,
  Dependabot/Renovate (GPUI is a pinned git revision bumped by hand), signed releases, SLSA-style build
  provenance where feasible.
- **Elevation**: none in v1.0. The app never asks for admin/root; protected-path operations
  fail with a clear message. A privileged helper is explicitly a non-goal until designed in an
  ADR.
- **Updates**: disabled in v1.0 builds; when enabled later, signed manifests with minisign and
  opt-in.
- **Threat model** (abridged): malicious filesystem content (weird names, sparse, devices,
  FIFOs) → treated as data, never opened; a bug in a view → limited by the plan/execute flow in `ferriscan-actions`; local attacker with write access to settings → out of scope beyond
  schema validation and size caps.

---

## 17. Testing and quality gates

### 17.1 Test layers

| Layer | Tool | Coverage target |
| --- | --- | --- |
| Engine unit tests | `cargo test` per crate | ≥ 85 % lines in `ferriscan-core` |
| Property tests | `proptest` on index invariants (size = Σ children, no cycles, arena bounds) | every invariant |
| Golden snapshots | `insta` on DTOs, layouts, reports | all public DTOs |
| FakeFs integration | synthetic trees: 1M files, 200k siblings, deep chains, unicode, symlink loops, permission errors | all scan paths |
| Platform integration | real temp dirs, real volumes where available, CI matrix | Windows/macOS/Linux |
| Action safety | table-driven plan/execute/undo scenarios incl. conflicts and TOCTOU | every branch |
| UI logic | `#[cfg(test)]` in `ferriscan-app` (layout cache, hit-testing, navigation, formatting) and GPUI `TestAppContext` | all pure view logic |
| Visual smoke | launch + `scripts/screenshot.ps1` | key screens, light and dark |
| Accessibility | manual checklist with a screen reader | zero blocking issues |
| Performance | Criterion + budget gate | see §6.3 |

### 17.2 CI matrix

- `fmt` → `clippy -D warnings` → `cargo deny` → `test` (Linux, Windows, macOS) →
  `i18n-check` → benchmarks (nightly + PR label `perf`).
- Every PR must keep locale completeness and budgets green.

### 17.3 Definition of done (per feature)

1. Spec references implemented and linked in the PR.
2. Unit + integration tests; failure paths covered.
3. i18n keys added in `en` and all shipped locales (or tracked in the translation issue).
4. Light/dark/high-contrast screenshots attached.
5. Keyboard path verified; axe clean.
6. Benchmarks unaffected or explained.
7. Docs/ADR updated if behavior deviates from this spec.

---

## 18. Packaging, updates and distribution

- **Windows**: NSIS per-user installer, MSI optional; x64 + arm64; code signing via Azure Trusted
  Signing or an OV certificate. No runtime prerequisite (no WebView2).
- **Linux**: AppImage (portable), `.deb`, `.rpm`; `.desktop` entry, AppStream metadata, icons at
  16–512 px; needs Vulkan, fontconfig, xkbcommon and Wayland/X11 client libraries.
- **macOS**: universal or per-arch `.dmg`; hardened runtime, notarization, `LSMinimumSystemVersion`
  11.0; microphone/camera/automation entitlements absent.
- **Versioning**: SemVer; `0.x` until feature-complete, `1.0.0` at milestone M12.
- **Channels**: stable only in v1.0; nightly builds from `main` for testers.
- **Reproducibility**: pinned toolchain via `rust-toolchain.toml`, `Cargo.lock` committed,
  build script records commit SHA and target triple in the About dialog.

---

## 19. Risks and mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Metadata APIs behave differently under antivirus/EDR filters | Scan errors, slowness | Error taxonomy, per-directory resilience, documented diagnostics, test on Defender/CrowdStrike-enabled VMs |
| Windows long paths and reparse points (OneDrive) | Wrong sizes, recursion loops | `\\?\` everywhere, reparse-point detection, loop guard, `FILE_ATTRIBUTE_REPARSE_POINT` handling |
| GPUI is pre-1.0 and tied to Zed | API breaks, Rust version pressure | Pinned git revision, bumps as their own task, ADR-0005 risk list |
| Accidental data loss from actions | Reputation-destroying | Plan/confirm/token, trash default, journaled undo, protected paths, extensive action tests |
| GPU/driver problems with GPUI rendering | App cannot draw | Documented minimums, fallback story to be tested (PLATFORM_MATRIX) |
| Localization drift | Half-translated UI | CI key check, `en` fallback, translation issue template |
| Scope creep toward monitoring/cloud | Delays v1.0 | Anti-goals in §1, roadmap gate reviews |
| macOS signing cost | Distribution friction | Unsigned dev builds documented; CI signs when secrets exist |

---

## 20. Glossary

- **Snapshot** — immutable result of one scan: node store + metadata.
- **Node** — one filesystem entry (file, directory, symlink) in the store.
- **Allocated size** — bytes actually used on disk (clusters / blocks).
- **Logical size** — sum of file lengths.
- **Plan** — a validated, expiring description of a file operation set.
- **Journal** — local write-ahead record enabling undo of moves/copies.
- **Budget** — a numeric performance threshold enforced in CI.
- **SoA** — structure of arrays; layout used by the node store for cache efficiency.

---

*Changes to this document require review by the Core and UI owners and, for architectural
changes, a new ADR in `docs/adr/`.*
