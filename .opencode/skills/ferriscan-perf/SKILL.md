---
name: ferriscan-perf
description: Performance budget workflow for FerriScan. Use when touching the scanner hot loop, node store, query paging, layout algorithms, IPC payloads, or when a benchmark regresses.
---

# FerriScan performance workflow

## Rules

- Measure before changing code. Record machine, toolchain, profile (`release`), and seed.
- One variable per experiment; three runs; report median and spread.
- Never loosen `bench/budgets.toml` to make CI pass. Budget changes need an ADR.
- Hot-path changes require a benchmark in the same PR.

## Budgets (SPEC §6.3)

| Benchmark | Budget |
| --- | --- |
| scan_synthetic_1m (warm) | ≤ 12 s, ≥ 90k entries/s |
| scan_synthetic_1m peak RSS | ≤ 110 MB |
| children_page_200k | ≤ 10 ms P95 |
| treemap_layout_5k | ≤ 25 ms P95 |
| sunburst_layout_5k | ≤ 10 ms P95 |
| snapshot_aggregate_1m | ≤ 250 ms |
| search_1m_substring | ≤ 80 ms P95 |
| IPC 500-row page | ≤ 120 KB |
| Memory per entry | ≤ 96 B (excl. name bytes) |

## Commands

```bash
cargo xtask fixtures --size 1m
cargo bench -p ferriscan-core -- --save-baseline before
# change one thing
cargo bench -p ferriscan-core -- --baseline before
cargo xtask budget
```

## Common wins, in order

1. Remove allocations in the per-entry loop (reuse the local entry buffer).
2. Reduce lock scope (batch ~512 nodes per writer lock; never hold across a syscall).
3. Improve locality (SoA access order, avoid pointer chasing during aggregation).
4. Trim IPC payloads (flat arrays, no repeated strings, page caps).
5. Only then micro-optimize arithmetic; keep determinism.

## Red flags

- Sorting inside the scan loop.
- `collect()` of millions of items in query paths.
- A `Mutex` around the whole store during enumeration.
- String formatting in hot paths (`format!`, `to_string`).
- Recomputing layout on every hover or selection change.
