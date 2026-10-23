+++
id = "M9-116"
title = "Approval cards show the observation thumbnail"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P1"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-106", "M9-115"]
docs = ["adr-0037"]
+++

## Problem

A human approving a click saw coordinates but not the pixels the model was
looking at. That made deny/allow decisions unnecessarily blind.

## Scope

- Cache the latest computer observation PNG in memory (bounded data URL).
- Attach it to input approvals whose observation id matches, and to the
  sensitive-window observation approval itself.
- Web `approval` SSE carries `image`; the card renders only `data:image/*` with a
  data-only `img-src` CSP.
- TUI keeps its text approval UI; no terminal image protocol is claimed.
- Keep the image out of JSONL and the attachment store.
- Cap the data URL at ~2 MB; larger frames are omitted.
- Tests assert approvals carry the image for click/type and sensitive observe.

## Acceptance

- [x] Click/type approvals carry the current observation thumbnail.
- [x] Sensitive observe approval carries the screenshot being released.
- [x] The web card renders data URLs only.
- [x] CSP allows data images, not remote images.
- [x] JSONL/store unchanged; no base64 persistence.
- [x] Oversize frames are omitted.
- [x] Tests/clippy pass.

## Notes

The thumbnail is advisory UI, not forensic evidence. `--computer-keep-frames`
still controls durable storage.
