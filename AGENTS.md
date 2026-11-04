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
| Stack | Rust 2024 (pinned 1.90), Tauri 2.12, Svelte 5 + Vite 8 + TypeScript |
| Engine | `crates/ferriscan-core` (pure), `ferriscan-platform` (OS), `ferriscan-actions` (mutations) |
| UI | `apps/desktop/src` (Svelte), shell in `apps/desktop/src-tauri` |
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
5. **The frontend sends node IDs, not paths.** Paths are resolved in Rust from the snapshot.
6. **Every user-visible string is localized.** No literal text in `.svelte` files outside
   `lib/i18n`. `xtask i18n-check` enforces it.
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

# Frontend
npm --prefix apps/desktop install
npm --prefix apps/desktop run check
npm --prefix apps/desktop run lint
npm --prefix apps/desktop run test

# App
npm --prefix apps/desktop run tauri dev
npm --prefix apps/desktop run tauri build

# Cross-cutting
cargo xtask i18n-check
cargo xtask contract
cargo xtask budget          # only when touching hot paths
```

Windows note: the WebView2 runtime is required; CI uses the evergreen bootstrapper. Linux
requires `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libssl-dev`, `librsvg2-dev`. macOS requires
Xcode command-line tools.

**Minimum verification before reporting a task done:** `cargo fmt --check`, `clippy -D warnings`,
`cargo test` for the touched crates, frontend `check` if TS/Svelte changed. State explicitly
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
- `unsafe` only in `ferriscan-platform` and the Tauri shell; each block has a `// SAFETY:`
  comment naming the invariant and the caller contract.
- No `println!` outside CLI/xtask; use `tracing`.
- Tests live in `#[cfg(test)] mod tests` for units and `tests/` for integration.
- Naming: `scan_dir` not `scanFolder`; `node_id` not `nodeID`.

### TypeScript / Svelte

- `strict: true`, no `any`, no non-null assertion without a comment.
- Svelte 5 runes only: `$state`, `$derived`, `$effect`, `$props`. No stores, no
  `createEventDispatcher`, no `export let`.
- Components: one concern, props typed, callbacks named `onX`.
- All IPC through `lib/ipc/client.ts`; no `invoke` in components.
- Styling: design tokens only. No hex colors, no px magic numbers outside `app.css`.
- Lists over 200 rows must use the virtualizer.
- Every async UI path has loading, empty and error states.

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
| Add a scan option | `core/src/scan/mod.rs`, `core/src/dto/scan.rs`, `src-tauri/src/commands/scan.rs`, UI scan panel, locales |
| Add a view | `core/src/query` + `core/src/layout`, `dto`, command, `components/views`, `state/view.svelte.ts`, locales, E2E |
| Touch OS behavior | `crates/ferriscan-platform/<area>/<os>.rs` + `docs/PLATFORM_MATRIX.md` |
| Change a destructive op | `crates/ferriscan-actions` + ADR-0003 review + safety tests |
| Add a setting | `persist/src/settings.rs`, `dto/settings.rs`, settings panel, locale keys, migration test |
| Add a locale | `apps/desktop/src/lib/i18n/locales/<tag>/`, registry, SPEC §14 |

## 6. Working protocol for agents

1. **Orient.** Read the task's milestone in `docs/ROADMAP.md`, the relevant SPEC sections, and
   the module you will touch. Do not start from assumptions about APIs — read the code.
2. **Plan.** For anything larger than a single file, write a short plan (bullets) into your
   response before editing. Name the files you expect to change.
3. **Implement in vertical slices.** Data model → engine → IPC → UI → test. Keep the workspace
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
- [ ] Frontend: `check`, `lint`, `test` when TS/Svelte changed.
- [ ] i18n keys added in `en` and tracked for other locales.
- [ ] Light / dark / high-contrast verified for UI changes (screenshots in PR).
- [ ] Keyboard path and accessible name verified for UI changes.
- [ ] Benchmarks unaffected, or delta explained.
- [ ] Docs/ADR updated when behavior deviates.
- [ ] No new network usage, no new `unsafe` outside allowed crates, no secrets.

## 8. Security and privacy guardrails

- Never log full paths at `info`; redact to a BLAKE3 prefix unless verbose logging is on.
- Never read file contents. Metadata only.
- Never add a webview capability for `fs`, `shell`, or unrestricted `http`.
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
| Svelte UI, design tokens, charts, a11y | `ui-engineer` |
| Locales, Fluent, formatting | `i18n-engineer` |
| Tests, CI, fixtures, E2E | `test-engineer` |
| Benchmarks, profiling, budgets | `perf-engineer` |
| Security review, threat model, supply chain | `security-reviewer` |
| Docs, README, guides, changelog | `docs-writer` |
| Installers, signing, release workflow | `release-engineer` |

`orchestrator` is the primary agent that plans milestones and delegates to the above.
