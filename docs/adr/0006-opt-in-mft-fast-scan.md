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

## Measured results (2026-11-09, `C:\`, 2.8M directory entries, NVMe SSD, release build)

| Mode | Time |
| --- | --- |
| Directory walk, unelevated | 15-20 s warm, about 40 s cold |
| MFT: read and index (8 threads, 4 MiB chunks) | 3.4-4.5 s (about 10 s on a cold first run) |
| MFT: engine scan over the index | 2.5-2.9 s (about 1M entries/s) |

Reading is limited by the disk (about 0.9 GB/s here); thread count and unbuffered I/O made no
difference. Per-subtree totals match the directory walk for `Documents`, `AppData` and
`Roaming` (counts within live-change noise). `System32` differs by 0.3 GiB because the walk hits
4 access errors that the MFT does not. Hard links are listed under every parent like the walk
does, and names kept in extension records are attached to their base file; both were bugs found
by this comparison.

## Alternatives considered

| Option | Why not |
| --- | --- |
| Tune the unelevated walk | Measured: thread count plateaus at 16, relative opens give no gain. |
| `FSCTL_ENUM_USN_DATA` | Also needs a volume handle and returns no sizes. |
| Elevated helper service | Larger attack surface and install complexity; explicitly a non-goal. |
