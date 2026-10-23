+++
id = "M9-088"
title = "Content-addressed attachment store"
status = "done"
milestone = "M9"
module = "context"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-087"]
docs = ["adr-0037"]
+++

## Problem

Images and scripts need a durable, deduplicated home outside the JSONL. Putting
base64 in events would bloat logs, backups, and exports; arbitrary filenames
would create traversal risk.

## Scope

- Add `niucode-attachments`.
- SHA-256 content addressing; atomic `0600` writes under `0700`.
- Detect PNG/JPEG/WebP and UTF-8 text; reject other binary.
- Per-file limits: 10 MiB image, 20k text chars.
- Per-turn budget: four files, 25% of context, 4k text tokens, 1024 tokens per
  image.
- Metadata reference stores hash/media/kind/bytes/chars/name/alt.
- No UI or event integration yet; no base64 in the event schema.

## Acceptance

- [x] Identical bytes deduplicate.
- [x] Files are `0600` and the directory is `0700` on Unix.
- [x] Text round-trips and oversize text is rejected.
- [x] Binary/NUL data is rejected.
- [x] Invalid hashes cannot escape the store.
- [x] Budget rejects too many files and oversize context.
- [x] Image data URLs are generated only from trusted references.
- [x] Package verifies.

## Notes

Wiring `AttachmentRef` into events and provider messages is the next slice.
