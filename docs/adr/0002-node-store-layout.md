# ADR-0002 — Structure-of-arrays node store

- **Status:** Accepted
- **Date:** 2026-11-04
- **Deciders:** Core
- **Related:** SPEC §9, FILE_ARCHITECTURE §3

## Context

A 4 TB volume can contain 5–20 million entries. The index must:

- fit in memory at ≤ 96 bytes per entry including overhead (NFR-3),
- allow appending millions of nodes during a parallel scan without per-node allocation,
- answer "children of X sorted by Y, page N" in ≤ 10 ms for 200k children,
- support path reconstruction and prefix/substring search,
- remain immutable after publish so the UI and queries never see torn state.

## Decision

Store nodes in a flat `Vec<NodeRecord>` (56 bytes, `#[repr(C)]`, index-linked via
`parent/first_child/next_sibling`), with names in a separate bump arena (`Vec<u8>`) referenced
by `(offset, len)`. Extensions are interned into a dictionary. Aggregates are computed once at
finalize. Directories above 8k children get an optional sorted index built at finalize.

Insertion during the scan is append-only; the sibling chain is preserved by an explicit
`last_sibling` scratch vector in the writer, not stored per node. Parent→children order is
insertion order, which is stable and reproducible across runs.

## Consequences

### Positive

- Predictable memory: 56 B + name bytes + ~4 B arena slack per node.
- No per-node heap allocation; arenas grow geometrically (amortized O(1)).
- Sequential scans over `Vec<NodeRecord>` are cache friendly for aggregation and layout.
- Immutability after publish removes locking from queries and guarantees consistent layouts.
- Path reconstruction is a parent walk; no path strings stored (also a privacy benefit).

### Negative

- Structural sharing after a file action requires cloning the store (acceptable: human-paced,
  chunked, and documented in SPEC FR-4.7).
- No cheap insert-in-sorted-position during scan; sorting is deferred.
- `u32` node cap (4.29 B) is ample but must be checked; exceeding it aborts the scan with
  `E_UNSUPPORTED` rather than corrupting.
- Debugging requires accessor helpers; raw indices are error-prone (mitigated by `NodeId`
  newtype, invariant checks in debug builds, and property tests).

### Neutral

- Names > 65,535 bytes are truncated for display and flagged; the OS path is still used for
  operations via a side table (maximum component length is 255 on all target filesystems, so
  this is effectively unreachable).

## Alternatives considered

| Option | Why rejected |
| --- | --- |
| `Vec<Node>` with `Vec<NodeId>` children | Extra indirection and 24 B/child allocation; cache misses during aggregation. |
| Arena of boxed trees (`Rc<RefCell>`) | Pointer chasing, refcount traffic, non-`Send` across scan threads. |
| Embedded graph database (sled, redb) | On-disk overhead, slower scans, unnecessary durability; results are ephemeral by design. |
| Storing full paths per node | Duplicated bytes (O(depth)), privacy risk, slower name search. |

## Follow-up

- [ ] T2.1 implementation + property tests for I1–I5.
- [ ] T7.4 verify the 96 B/entry ceiling on the 5M-entry profile.
- [ ] Revisit structural sharing only if post-action patching exceeds 500 ms on a 5M-entry tree.
