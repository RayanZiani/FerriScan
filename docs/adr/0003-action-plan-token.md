# ADR-0003 — Plan + HMAC token for every filesystem mutation

- **Status:** Accepted. Open question after ADR-0005: the token was designed against a compromised webview; the native UI has no such renderer and currently runs plan then execute without the token. `security-reviewer` must decide whether to keep, adapt or retire it.
- **Date:** 2026-11-04
- **Deciders:** Core, Security
- **Related:** SPEC §12, FR-4.2/FR-4.5

## Context

FerriScan can delete and move user data. The most likely catastrophic bug is not a bad
visualization but a half-executed destructive action, a stale selection, or a compromised
webview invoking `execute_action` with attacker-chosen arguments.

We need: a single reviewable description of intent, human confirmation that matches what will
happen, protection against the window between review and execution (TOCTOU), and an audit trail.

## Decision

Every mutating operation is two-phase:

1. `plan_action(request)` is **pure** — it reads the filesystem but never writes. It resolves
   canonical paths, expands selected node IDs into filesystem items, detects conflicts,
   permissions, free space and protected paths, and returns an `ActionPlan` with a random
   `planId`, an expiry (5 min), and a `confirmToken = HMAC-SHA256(secret, planId || canonical(plan))`
   truncated to 32 bytes (base64url). The secret is generated per process start and never
   persisted.
2. `execute_action({ planId, confirmToken })` — the backend looks up the plan, recomputes the
   token over the stored plan (constant-time compare), checks expiry, re-validates each item's
   `(kind, size, mtime)` snapshot, then executes sequentially, writing a journal entry before
   the first write.

The frontend never sends paths: it sends node IDs, and the backend derives paths from its own
snapshot. The plan is displayed from the same serialized structure that was hashed, so the user
reviews exactly what executes.

## Consequences

### Positive

- Eliminates path injection and "hidden extra item" attacks from the webview.
- TOCTOU becomes explicit (`E_CHANGED`) instead of silently destructive.
- Plans are serializable, testable and loggable (test suite asserts plan output tables).
- Undo is natural: the journal is the executed plan plus results.

### Negative

- Two round-trips per action; the UI must hold plan state and handle expiry.
- Long-running plans (millions of files) can exceed the 5-minute window; large plans use a
  long-lived token scoped to a single execution and are chunked with progress + revalidation
  per chunk.
- A process restart invalidates plans (by design).

### Neutral

- The HMAC adds no confidentiality; it is an integrity/authorization binding, not encryption.
- Plan caching is bounded (8 plans, LRU) with a zeroizing drop.

## Alternatives considered

| Option | Why rejected |
| --- | --- |
| Direct `delete(paths)` command | No review step, trivially abusable from a compromised webview. |
| Confirm boolean passed by frontend | Attacker-controlled; no binding to the reviewed plan. |
| Native OS confirmation only | Inconsistent across platforms, no per-item conflict resolution, poor a11y. |
| Signed plan stored on disk | Unnecessary persistence of paths; process-scoped secret is sufficient. |

## Follow-up

- [ ] T5.1/T5.2 implementation with constant-time compare.
- [ ] Adversarial tests: mutated token, expired plan, replayed plan, plan from another process.
- [ ] Chunked execution design for > 100k items (T5.3).
