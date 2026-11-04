# Security Policy

## Supported versions

FerriScan is pre-1.0. Security fixes land on `main` and the latest release only.

| Version | Supported |
| --- | --- |
| Latest release | Yes |
| `main` | Yes (development) |
| Older | No |

## Reporting a vulnerability

**Do not open a public issue.** Use GitHub's private vulnerability reporting
(Security → Report a vulnerability) or email the maintainers listed in `README.md`.

Include:

- Affected version and platform.
- A description of the impact (data loss, path escape, code execution, privacy leak…).
- Reproduction steps or a minimal proof of concept.
- Whether the issue requires local access, a malicious filesystem, or a compromised webview.

You can expect an acknowledgement within 72 hours and a remediation plan within 14 days for
confirmed issues. Credit is given in the release notes unless you prefer otherwise.

## Scope

High-value areas:

- `crates/ferriscan-actions` — any path that could delete or move data the user did not confirm.
- The IPC boundary — any way for the webview to supply a path or escape the node-ID model.
- `ferriscan-platform` `unsafe` code and path handling (`\\?\`, reparse points, symlinks).
- Packaging and update artifacts (signature verification, installer behavior).

Out of scope: issues requiring an already-compromised OS account with write access to the
user's config directory, and social-engineering attacks.

## Design commitments

- No telemetry, no accounts, no network access in v1.0 (see `docs/adr/0004-no-telemetry.md`).
- Every destructive action goes through a plan with a short-lived HMAC token
  (see `docs/adr/0003-action-plan-token.md`).
- No shell execution; external programs are invoked with explicit argv.
- Minimal Tauri capabilities: no `fs`, no `shell`, no unrestricted `http`.
