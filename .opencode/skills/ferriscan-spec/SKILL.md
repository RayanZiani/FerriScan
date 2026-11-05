---
name: ferriscan-spec
description: Traceability and consistency checks for FerriScan work. Use when implementing, reviewing, or planning any FerriScan change, to map behavior to SPEC requirement IDs and roadmap tasks and to catch spec drift.
---

# FerriScan spec traceability

Use this checklist before and after any FerriScan change.

## Before

1. Find the requirement ID(s) in `docs/SPEC.md` (FR-x.y, NFR-x) that the change implements.
2. Find the roadmap task in `docs/ROADMAP.md` (Tx.y) and its milestone exit criteria.
3. Confirm the owning crate/module in `docs/FILE_ARCHITECTURE.md` §12.
4. If the change spans more than six areas or alters a budget/contract/security rule, stop and
   request an ADR from `architect`.

## After

- [ ] Behavior matches the cited requirement, including error paths.
- [ ] Tests reference the requirement ID in a comment or test name.
- [ ] Budgets unaffected, or the delta is measured and explained.
- [ ] i18n keys exist for every user-visible string.
- [ ] Platform differences recorded in `docs/PLATFORM_MATRIX.md`.
- [ ] No network code, no shell execution, no new `unsafe` outside allowed crates.

## Drift signals (escalate, do not silently fix)

- The code needs a field the DTO does not have.
- Two crates both seem to own the behavior.
- A limit in the spec (page size, search cap, name length) must be raised.
- A safety rule (plan/token, protected paths, trash default) would be bypassed "temporarily".
