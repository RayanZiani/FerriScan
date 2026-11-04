# ADR-0004 — No telemetry, no account, no network in v1.0

- **Status:** Accepted
- **Date:** 2026-11-04
- **Deciders:** Product, Core
- **Related:** SPEC §13, `docs/PRIVACY.md`

## Context

A disk analyzer enumerates every file name on a machine. That data is unusually sensitive:
project names, client folders, medical scans, personal photos. Users installing a "cleaner"
tool are often already in a stressful situation (disk full, failing drive).

At the same time, we want to know where the app crashes and which features are used.

## Decision

v1.0 ships with **zero** telemetry, zero analytics, zero accounts, and no outbound network
requests. Diagnostics are opt-in and manual: the user can open the log folder and attach logs
to a GitHub issue, with paths redacted by default. The update check is disabled in v1.0 builds.

Any future change to this requires a new ADR, a prominent changelog entry, an opt-in default,
and a documented data inventory update.

## Consequences

### Positive

- The privacy claim is verifiable by reading `Cargo.toml`/`package.json` and by network capture.
- Entering a regulated environment (hospital, law firm, public administration) needs no DPA.
- No third-party SDK supply-chain risk, no consent UI, no cookie/tracking complexity.
- Marketing and trust story is simple and honest.

### Negative

- We lose passive crash clustering and feature usage metrics; bugs are found through issues,
  beta feedback and local reproductions.
- Performance regressions depend on our own benchmarks, not field data.
- Priorities are guided by qualitative feedback rather than funnels.

### Neutral

- A future opt-in update check must be designed carefully (signed manifest, no identifiers,
  no server-side logging) and is explicitly out of scope for v1.0.

## Alternatives considered

| Option | Why rejected |
| --- | --- |
| Anonymous usage counters (opt-out) | Violates the core promise; file-related events risk leaking paths. |
| Crash reporting with minidumps | Minidumps can contain memory with paths; needs a consent flow and a backend. |
| Self-hosted analytics | Still a network path and operational burden for a small project. |
| Local-only diagnostics bundle | Adopted — as an explicit user action, not automatic upload. |

## Follow-up

- [ ] CI network-egress audit of packaged binaries (`strace`/`mitmproxy` smoke).
- [ ] `docs/PRIVACY.md` kept in sync with every release.
- [ ] Add a "Verify privacy" section in the About dialog pointing at the source.
