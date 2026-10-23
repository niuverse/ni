+++
id = "M9-093"
title = "Web image and script attachments"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-092"]
docs = ["adr-0037", "adr-0036"]
+++

## Problem

The web composer could only send text; the kernel attachment store and provider
image parts were unreachable from the browser.

## Scope

- Add `POST /api/attach` (base64 JSON) that classifies, limits, hashes, and
  stores under the current session's attachment directory.
- `POST /api/prompt` accepts validated attachment refs, checks the budget, and
  uses `run_with_attachments`.
- Raise the local request cap to 16MiB for base64 image uploads.
- Add Attach button, file picker, drag/drop, clipboard image paste, pending
  chips, remove controls, and clear-after-send.
- Tests: upload endpoint, prompt branch with image part, static client markers.

## Acceptance

- [x] Upload returns a stored `AttachmentRef`.
- [x] Invalid base64 is 400; invalid refs are 400.
- [x] Prompt with an uploaded image reaches the provider as image parts.
- [x] Budget is checked server-side.
- [x] Chips show kind/name and can be removed.
- [x] Existing token/Host/Origin/CSP guards still apply.
- [x] Static/HTTP tests pass.

## Notes

`/api/attach` is local-only and token-gated. Text attachments are accepted by the
same endpoint (UTF-8, ≤20k chars). No browser-side resize; oversize is rejected.
