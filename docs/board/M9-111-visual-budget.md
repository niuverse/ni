+++
id = "M9-111"
title = "Four-image visual budget per provider request"
status = "done"
milestone = "M9"
module = "loop"
size = "S"
priority = "P1"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-090", "M9-099"]
docs = ["adr-0037"]
+++

## Problem

Recent history (2), current attachments (up to 4), and a computer screenshot
could combine into more than the four-image visual budget in ADR-0037. The
provider request was unbounded.

## Scope

- Add a transport-level cap of four image parts per provider request.
- Replace the oldest images with `[image omitted: visual budget]` text; keep
  the newest images and any metadata text.
- Run the cap before every provider request, and reserve one slot before adding
  a computer screenshot.
- Leave JSONL events and the attachment store untouched; this is a per-request
  projection, not deletion.
- Unit tests for drop order and an agent request-level regression.

## Acceptance

- [x] Every provider request carries at most four images.
- [x] Oldest images are dropped first.
- [x] A screenshot reserves one slot.
- [x] Tool/user metadata text survives.
- [x] Events and files are not rewritten.
- [x] Tests/clippy pass.

## Notes

The marker is visible to the model; it does not claim the file disappeared from
storage. Attachment-store cleanup remains a separate operation.
