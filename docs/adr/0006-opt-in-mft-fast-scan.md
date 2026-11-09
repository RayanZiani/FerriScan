# ADR-0006 - Opt-in elevated MFT fast scan on Windows

- **Status:** Accepted
- **Date:** 2026-11-09
- **Deciders:** Core, Platform
- **Related:** ADR-0005 (scan speed measurements), SPEC section 16 (elevation)

## Context

An unelevated directory walk on Windows is bounded by the kernel: on `C:\` (2.77M entries,
435k directories) it takes 15-20 s warm and about 40 s cold, 94 % of it in kernel time spent
opening and enumerating directories (ADR-0005, "Scan speed, whole system drive"). TreeSize and
WizTree reach roughly 15 s per terabyte or less by reading the NTFS Master File Table (MFT)
sequentially, which needs a read handle on the volume and therefore administrator rights.

## Decision

- Keep the unelevated walk as the default and the fallback for every volume and filesystem.
- Add a read-only MFT reader in `ferriscan-platform` (Windows, NTFS only) that opens the
  volume, reads the MFT in large sequential chunks, applies fixups, extracts name, parent,
  sizes, attributes and mtime, and exposes an in-memory index through `DirReader`
  (`read_dir_keyed`). `ferriscan-core` scanning, aggregation and snapshots are unchanged.
- The app never elevates silently. When not elevated it offers an explicit action that
  relaunches the whole app with the `runas` verb (UAC prompt) and closes the current
  instance. An elevated instance uses the MFT reader for NTFS volume roots automatically.
- No privileged helper, service or IPC. Scanning a sub-folder keeps using the directory walk.
- The MFT is only read; file contents are never read and nothing is written to the volume.
  File mutations still go exclusively through `ferriscan-actions`.

## Consequences

- Faster scans of whole NTFS volumes for users who accept the UAC prompt.
- `unsafe` volume I/O stays inside `ferriscan-platform`. The record parser is pure and unit
  tested with synthetic records; the volume I/O needs an elevated manual check.
- Elevated runs can see files the user could not open; results still go through the same plan
  and review flow before any change.
- Extra risk: parsing untrusted on-disk structures. The parser bounds-checks every offset and
  skips malformed records instead of failing the scan.

## Alternatives considered

| Option | Why not |
| --- | --- |
| Tune the unelevated walk | Measured: thread count plateaus at 16, relative opens give no gain. |
| `FSCTL_ENUM_USN_DATA` | Also needs a volume handle and returns no sizes. |
| Elevated helper service | Larger attack surface and install complexity; explicitly a non-goal. |
