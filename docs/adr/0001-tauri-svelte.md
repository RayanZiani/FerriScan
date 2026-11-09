# ADR-0001 — Tauri 2 + Svelte 5 for the desktop shell

- **Status:** Superseded by ADR-0005 (GPUI native UI)
- **Date:** 2026-11-04
- **Deciders:** Core, UI
- **Related:** SPEC §7, FILE_ARCHITECTURE §2

## Context

FerriScan needs a native desktop UI on Windows, Linux and macOS, with a heavy Rust engine, a
rich visualization layer (canvas treemaps at thousands of nodes), native theme integration, and
a small installer. The engine is inherently Rust (OS metadata APIs, performance budgets of
§6.3). The UI layer is the open question.

Constraints:

- Installer ≤ 15 MB, installed footprint ≤ 40 MB (NFR-7).
- 60 fps with virtualized 100k-row lists and 5k-node canvases.
- Native theme/locale discovery and window chrome that follows light/dark.
- Cross-platform packaging and signing with limited maintainer time.
- No network access at runtime; strict local security posture.

## Decision

Use **Tauri 2** as the shell and **Svelte 5 (runes) + Vite + TypeScript** as the UI, rendering
treemap/sunburst on Canvas 2D and the tree table with a custom virtualizer. Rust owns all data
and all file operations; the webview receives paged DTOs and precomputed layout arrays only.

## Consequences

### Positive

- Binaries ~5–10 MB (vs ~100 MB Electron) and low idle memory.
- The engine runs in the same process as the shell; no sidecar IPC.
- WebView2/WebKitGTK/WKWebView give native text rendering, IME and accessibility primitives.
- Svelte's compiled output and fine-grained reactivity avoid virtual-DOM overhead on large lists.
- Mature Tauri plugins for dialogs and "open with" without exposing filesystem APIs.

### Negative

- WebView differences across platforms (WebView2 vs WebKitGTK vs WKWebView) require per-OS
  testing, especially for canvas performance and font metrics.
- WebKitGTK version skew on older Linux distros; we must document a minimum.
- Rust + JS toolchain and two package managers add setup friction for contributors.
- E2E tooling (`tauri-driver`) is less mature than Playwright-on-Chromium; some journeys need
  manual verification on macOS.

### Neutral

- Strict CSP and a minimal capability allowlist become mandatory practices (SPEC §16).
- The IPC boundary needs a contract test to stay honest (FILE_ARCHITECTURE §10).

## Alternatives considered

| Option | Why rejected |
| --- | --- |
| **Electron** | ~100 MB baseline, high RAM, Chromium update treadmill, contradicts NFR-7/NFR-8. |
| **egui** | Excellent for tools, weak for a polished, themed, accessible consumer UI; text/layout quality and i18n fall short. |
| **iced** | Promising but young ecosystem, limited accessibility story, heavy custom widget work for tables/treemaps. |
| **Qt (C++/QML via cxx-qt)** | Great native widgets and tables, but licensing complexity, large runtime, and a C++ build dependency. |
| **Slint** | Very good embedded story; smaller ecosystem, licensing for some features, less HTML-class layout flexibility. |
| **Web app + local server** | Port conflicts, browser security prompts, no native chrome/theme, worse file access story. |

## Follow-up

- [ ] T0.3 scaffold; validate canvas perf on WebKitGTK in M4.
- [ ] Document minimum WebKitGTK in `docs/PLATFORM_MATRIX.md`.
- [ ] Decide `specta` vs hand-written DTO mirrors at T3.5 (separate ADR if we deviate).
