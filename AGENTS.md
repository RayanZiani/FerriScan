# AGENTS.md — FerriScan engineering rules

This file is read by every agent and human contributor. It is **normative**. If a task
conflicts with it, stop and raise the conflict instead of improvising.

Read in this order before touching code:

1. `docs/SPEC.md` — what the product is and how it must behave.
2. `docs/FILE_ARCHITECTURE.md` — where code lives and which crate owns what.
3. `docs/ROADMAP.md` — current milestone and task IDs.
4. This file — how we work.

---

## 1. Project facts

| | |
| --- | --- |
| Product | FerriScan — fast, private, open-source disk space analyzer |
| License | MIT |
| Platforms | Windows 10 1809+ (primary), Linux, macOS 11+ |
| Stack | Rust 2024 (pinned 1.98.1), `GPUI` (pinned Zed revision, ADR-0005). 100 % Rust, no webview, no Node |
| Engine | `crates/ferriscan-core` (pure), `ferriscan-platform` (OS), `ferriscan-actions` (mutations) |
| UI | `apps/desktop` (`ferriscan-app`, GPUI) |
| Privacy | No account, no telemetry, no network in v1.0 |
| Docs language | English, everywhere — code, comments, commits, issues, docs |

## 2. Non-negotiable principles

1. **Performance is a feature with a number.** Every hot path has a budget in SPEC §6.3. Do not
   merge a change that breaks a budget; do not "optimize later" a data structure that the spec
   already fixed.
2. **The engine has no UI and no OS.** `ferriscan-core` must compile and pass tests with
   `--no-default-features` on any machine. OS calls go through traits.
3. **Only `ferriscan-actions` mutates user files.** No exceptions, including "temporary" code.
4. **No shell.** Never `sh -c`, `cmd /c`, or interpolated commands. `Command::new(program).args([...])`.
5. **The UI holds node IDs, not paths.** Paths are resolved from the snapshot at the point of use.
6. **Every user-visible string is localized.** No literal text in UI code: use
   `ferriscan_i18n::Catalog` keys from `crates/ferriscan-i18n/locales`. `xtask i18n-check` will
   enforce it once implemented.
7. **Privacy is structural.** Adding `reqwest`, an analytics SDK, or any outbound call is a
   spec violation, not a review comment.
8. **Immutability after publish.** A `Snapshot` is never mutated; patches create a new one.
9. **No `unwrap`/`expect` in library code** except after a proven invariant, with a comment.
   Tests may unwrap freely.
10. **Determinism.** Scans, layouts, exports and sorts must be reproducible for the same input.

## 3. Build and verify

```bash
# Backend
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check

# App
cargo run -p ferriscan-app              # run the desktop app
cargo build --release -p ferriscan-app

# Cross-cutting
cargo xtask i18n-check
cargo xtask budget          # only when touching hot paths
```

Windows needs the Visual Studio Build Tools (C++ workload). Linux needs the GPUI system
libraries: `libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libxcb1-dev libx11-xcb-dev
libfontconfig-dev libvulkan-dev`. macOS requires Xcode command-line tools. GPUI needs Rust
1.98+, which is why the toolchain is pinned to 1.98.1.

**Minimum verification before reporting a task done:** `cargo fmt --check`, `clippy -D warnings`,
`cargo test` for the touched crates. State explicitly
which commands you ran and their result.

## 4. Code conventions

### Rust

- Edition 2024, `#[deny(unsafe_op_in_unsafe_fn)]`, workspace lints on.
- Errors: `thiserror` in libraries, `anyhow` only in CLI/xtask. Public APIs never return
  `anyhow::Error`.
- Types over primitives at boundaries: `NodeId`, `VolumeId`, `SnapshotId`, `ByteSize` (newtype
  optional), `ErrorCode`.
- Modules: `foo.rs` + `foo/` directory; no `mod.rs`.
- Every public item has a doc comment explaining *why*, not restating the signature.
- `unsafe` only in `ferriscan-platform`; each block has a `// SAFETY:`
  comment naming the invariant and the caller contract.
- No `println!` outside CLI/xtask; use `tracing`.
- Tests live in `#[cfg(test)] mod tests` for units and `tests/` for integration.
- Naming: `scan_dir` not `scanFolder`; `node_id` not `nodeID`.

### UI (GPUI)

- One concern per view or widget; state lives in the `Shell` entity or a small struct it owns.
- No blocking work in `render`: OS and engine calls go through `cx.background_executor()` and
  results come back with `this.update`.
- Colors and spacing come from `theme.rs` tokens; no ad-hoc hex values in views.
- Lists that can exceed 200 rows use `uniform_list`.
- Every async UI path has loading, empty and error states (toast on failure).
- Closures stored by GPUI are `'static`: clone `Arc`/`Rc` handles instead of borrowing the view.

### Commits

Conventional Commits, imperative, ≤ 72 chars subject:

```
feat(scan): cancel in-flight directory reads within 250 ms
fix(actions): refuse cross-volume move when free space is insufficient
docs(spec): clarify allocated size on compressed NTFS files
perf(core): avoid per-entry allocation in the scan hot loop
test(actions): cover TOCTOU replacement between plan and execute
chore(ci): pin windows runner image
```

Body: what changed, why, which task ID (`T2.4`), and how it was verified. One logical change per
commit. Never commit generated fixtures, scan results, or secrets.

### Branch and PR

- Branch: `feat/m3-children-query`, `fix/t5-toctou`, `docs/adr-0005`.
- PR title mirrors the commit convention and includes the task ID.
- PR body template: Summary / Spec references / How verified / Screenshots (UI) / Risks.
- CI must be green; no `--no-verify`, no skipped tests without an issue link.

## 5. Where to put things

Follow `docs/FILE_ARCHITECTURE.md` §12 ("Adding a feature: which files change"). If your change
spans more than six areas, write an ADR first.

| I need to… | Go to |
| --- | --- |
| Add a scan option | `core/src/scan/mod.rs`, `core/src/dto/scan.rs`, `apps/desktop/src/shell.rs` (scan panel), locales |
| Add a view | `core/src/query` + `core/src/layout`, view in `apps/desktop/src`, locales, tests |
| Touch OS behavior | `crates/ferriscan-platform/<area>/<os>.rs` + `docs/PLATFORM_MATRIX.md` |
| Change a destructive op | `crates/ferriscan-actions` + ADR-0003 review + safety tests |
| Add a setting | `persist/src/settings.rs`, `dto/settings.rs`, settings panel, locale keys, migration test |
| Add a locale | `apps/desktop/src/lib/i18n/locales/<tag>/`, registry, SPEC §14 |

## 6. Working protocol for agents

1. **Orient.** Read the task's milestone in `docs/ROADMAP.md`, the relevant SPEC sections, and
   the module you will touch. Do not start from assumptions about APIs — read the code.
2. **Plan.** For anything larger than a single file, write a short plan (bullets) into your
   response before editing. Name the files you expect to change.
3. **Implement in vertical slices.** Data model → engine → UI → test. Keep the workspace
   compiling between slices.
4. **Test as you go.** Add the test in the same change as the behavior. Failure paths are part
   of the behavior.
5. **Verify.** Run the commands in §3. Fix warnings; do not silence them.
6. **Report.** What changed, files touched, commands run with results, what remains, and any
   spec deviation (with the ADR you wrote).

Rules of engagement:

- **Do not** invent requirements. If the spec is silent, implement the smallest reasonable
  behavior and note it in the PR; if it is architectural, propose an ADR.
- **Do not** rewrite unrelated code, reformat untouched files, or upgrade dependencies
  opportunistically. Dependency upgrades are their own task.
- **Do not** delete or "clean up" files you did not create without checking `git log`/`git status`
  and asking when in doubt.
- **Do not** add a dependency that is not in the allowlist in `FILE_ARCHITECTURE.md` §2 without
  justification in the PR (binary size, license, maintenance).
- **Do not** commit unless asked. Leave changes in the working tree and report.
- **Do not** run destructive filesystem operations outside test sandboxes.
- **Stop and ask** when: a spec conflict appears, two crates both seem to own a behavior, a
  performance budget cannot be met, or a change requires elevated privileges.

## 7. Definition of done (checklist)

- [ ] Behavior matches the cited SPEC requirement IDs.
- [ ] Unit + integration tests, including error paths.
- [ ] `cargo fmt`, `clippy -D warnings`, `cargo test` for touched crates.
- [ ] i18n keys added in `en` and tracked for other locales.
- [ ] Light / dark / high-contrast verified for UI changes (screenshots in PR).
- [ ] Keyboard path and accessible name verified for UI changes.
- [ ] Benchmarks unaffected, or delta explained.
- [ ] Docs/ADR updated when behavior deviates.
- [ ] No new network usage, no new `unsafe` outside allowed crates, no secrets.

## 8. Security and privacy guardrails

- Never log full paths at `info`; redact to a BLAKE3 prefix unless verbose logging is on.
- Never read file contents. Metadata only.
- Never add a webview or an embedded browser engine; the UI is native GPUI only.
- Validate every DTO with size caps (name ≤ 65,535 bytes, page ≤ 500 rows, search ≤ 1,000 hits).
- Plan tokens: constant-time comparison, 5-minute expiry, process-scoped secret.
- Any new external process invocation needs a review by `security-reviewer` and an entry in the
  allowlist with its exact argv shape.
- Report security issues per `SECURITY.md` (do not open a public issue).

## 9. Domain glossary (short)

Snapshot, node, plan, journal, budget, allocated vs logical size: see `docs/SPEC.md` §20.

## 10. Ownership map for agents

| Area | Agent |
| --- | --- |
| Spec, ADRs, cross-crate decisions | `architect` |
| `ferriscan-core` (model, scan, query, layout, DTO) | `core-engineer` |
| `ferriscan-platform`, OS quirks, packaging deps | `platform-engineer` |
| `ferriscan-actions` (plan/execute/undo/safety) | `actions-engineer` |
| GPUI UI, design tokens, charts, a11y | `ui-engineer` |
| Locales, Fluent, formatting | `i18n-engineer` |
| Tests, CI, fixtures, E2E | `test-engineer` |
| Benchmarks, profiling, budgets | `perf-engineer` |
| Security review, threat model, supply chain | `security-reviewer` |
| Docs, README, guides, changelog | `docs-writer` |
| Installers, signing, release workflow | `release-engineer` |

`orchestrator` is the primary agent that plans milestones and delegates to the above.
