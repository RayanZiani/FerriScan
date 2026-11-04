# FerriScan — Privacy

FerriScan has no account, no telemetry, no analytics, no crash upload, and no network code in
its scanning or analysis path. This page is the complete inventory of what exists on your
machine and why.

## Data inventory

| Data | Where | Why | Retention | Leaves the machine? |
| --- | --- | --- | --- | --- |
| Preferences (theme, language, units, scan defaults, excludes, column widths, window geometry) | OS config dir, `settings.json` | Remember your choices | Until you reset them or uninstall | No |
| Recent volume **labels** (opt-out) | same `settings.json` | Convenience shortcut list | Same | No |
| Action journal (paths of files you moved/copied, not deleted files' contents) | OS app-data dir, `journal/*.json` | Undo and transparency | 30 days or 50 entries; clearable | No |
| Logs (no paths at `info` level) | OS log dir, rotating | Diagnose bugs you report | 5 files × 1 MB | No (only if *you* attach them to a bug report) |
| Optional snapshot cache | OS cache dir | Faster reopening | Deleted on exit unless "keep cache" is on | No |
| In-memory scan snapshot (names, sizes, dates) | RAM | The whole point of the app | Until app closes or new scan | No |

## What FerriScan never does

- Never asks for an email, name, or account.
- Never sends file names, paths, sizes, hashes, or statistics anywhere.
- Never reads file *contents* — only metadata (name, size, timestamps, attributes).
- Never uploads logs or crash dumps automatically.
- Never includes a third-party analytics or advertising SDK.
- Never bundles a remote update check in v1.0 builds.

## Network usage

v1.0 makes **no outbound network requests**. The only network volumes it touches are those
already mounted by your operating system, and it reads them exactly like a local disk (metadata
only, read-only).

A future optional update check will be opt-in, documented, and off by default.

## Deleting your data

- **Settings → Privacy → Reset all settings** removes preferences.
- **Settings → Privacy → Clear activity history** deletes journal files.
- **Settings → Privacy → Clear cache** deletes the optional snapshot cache.
- Uninstalling removes the app; config/journal/cache directories can be deleted manually and
  their exact paths are shown in Settings → About.

## Verifying these claims

The source is MIT-licensed. Search the repository for `reqwest`, `hyper`, `ureq`, analytics
domains, or `http` in `Cargo.toml` files: none should appear outside of dev-dependencies used
for local tests. CI runs `cargo deny` and a network-egress audit on the packaged app.

## Children

FerriScan is not directed at children and collects no personal data from anyone.
