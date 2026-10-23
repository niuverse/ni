+++
id = "M9-089"
title = "Attachment references in user events"
status = "done"
milestone = "M9"
module = "context"
size = "S"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-088"]
docs = ["adr-0037"]
+++

## Problem

The attachment store had no durable association with a user turn; base64 in
JSONL was explicitly forbidden.

## Scope

- Move `AttachmentRef` / `AttachmentKind` into `niucode-core`.
- Add `attachments` to `Event::UserMessage`, defaulting empty and omitted from
  JSON when empty.
- Keep old logs readable.
- Add round-trip and backward-compatibility tests.
- No runtime/provider injection yet.

## Acceptance

- [x] Old user_message JSON without attachments decodes.
- [x] Empty attachments are not serialized.
- [x] Metadata round-trips without image data.
- [x] Store references the core type; no duplicate schema.
- [x] Workspace tests/clippy pass.

## Notes

Runtime injection of image data URLs and recent-image projection is the next
slice. Event logs still contain no base64.
