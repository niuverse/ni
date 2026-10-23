+++
id = "M9-107"
title = "Redact typed computer text in the event log"
status = "done"
milestone = "M9"
module = "loop"
size = "S"
priority = "P0"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-099"]
docs = ["adr-0037"]
+++

## Problem

The assistant tool-call event stored the complete JSON arguments for `computer
type`, so typed passwords/secrets landed in the append-only JSONL in plaintext.
ADR-0037 already required length + SHA-256 by default.

## Scope

- Before appending `AssistantMessage`, redact `computer`/`type` tool calls.
- Replace `text` with `[redacted: N chars sha256:…]`, add `text_chars` and
  `text_sha256`, and leave all other actions untouched.
- Keep the live tool call unredacted for XTEST execution and the in-memory
  provider conversation.
- Add a fake-backend regression with a secret-looking payload and assert neither
  the assistant call nor the ToolResult contains it.
- No opt-in full-text flag in this slice; full text is never persisted.

## Acceptance

- [x] JSONL has no plaintext typed text.
- [x] Length and SHA-256 are stable and recorded.
- [x] XTEST still receives the original text.
- [x] Non-type computer calls are unchanged.
- [x] ToolResult remains free of typed text.
- [x] Tests/clippy pass.

## Notes

Older logs may contain prior plaintext; this change does not rewrite history.
A future `--computer-log-input` can opt in explicitly, off by default.
