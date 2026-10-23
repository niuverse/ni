+++
id = "M9-040"
title = "Download the web session as Markdown"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-007"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The browser could read a session but not take it anywhere. Users had to find the
JSONL on disk and convert it themselves.

## Scope

- `GET /api/export` derives Markdown from the append-only log.
- Include cwd/model/provider, user/assistant text, tool outputs in four-backtick
  fences, token usage, and compaction summaries.
- Serve `text/markdown` with an attachment filename.
- Add a header Download button that fetches and saves it locally.
- Return a JSON 500 if the session cannot be loaded.
- No new file, store, or mutation.

## Acceptance

- [x] Export has a title and session metadata.
- [x] User/assistant/tool output are preserved.
- [x] Tool output fences tolerate embedded triple backticks.
- [x] The response has an attachment filename.
- [x] Browser button downloads the blob.
- [x] Unit tests cover the Markdown shape.

## Notes

The export is a projection, never a second source of truth. Durable session
formats and replay semantics are unchanged.
