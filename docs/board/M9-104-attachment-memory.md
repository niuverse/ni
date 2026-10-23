+++
id = "M9-104"
title = "Attachment memory across a store reopen"
status = "done"
milestone = "M9"
module = "context"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-090"]
docs = ["adr-0037"]
+++

## Problem

Attachment projection used an open store, but there was no evidence that a
session resumed in a new process could rebuild its recent-image history from
the content-addressed store alone.

## Scope

- Add an integration test that stores a PNG, runs an attached turn, drops the
  store, reopens it, and rebuilds history.
- Assert exactly one recent image part survives and is a `data:image/png` URL.
- Start a second session from that projected history and assert the provider
  request still contains the image.
- Export `project_history_with_attachments` for embedders/tests.
- No new storage format; screenshots remain ephemeral.

## Acceptance

- [x] Reopen from the same root restores the image.
- [x] Projection emits a recent-image part, not a path.
- [x] A second session's provider request keeps the image.
- [x] The original file is not needed after `put`.
- [x] Tests/clippy pass.

## Notes

This is C-layer evidence. Cross-session Markdown B is covered separately by
M10-020. Computer observation screenshots still do not enter the attachment
store by default.
