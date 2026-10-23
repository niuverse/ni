+++
id = "M9-072"
title = "Export tool-call arguments"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-040"]
docs = ["adr-0036"]
+++

## Problem

The Markdown export included assistant prose and tool results, but not the tool
calls' arguments. A transcript lost the actual path/command that produced the
output.

## Scope

- After assistant text, render each `tool_calls` entry as
  `### Tool call · name` plus pretty JSON in a four-backtick fence.
- Keep tool result output as its own section.
- No event-log or API change.
- Extend the export test with a tool call and path assertion.

## Acceptance

- [x] Export includes tool call names.
- [x] Export includes pretty input JSON.
- [x] Four-backtick fences survive embedded triple backticks.
- [x] Existing export/message sections remain.
- [x] Tests pass.

## Notes

The export still cannot join a result to its call id in the visible transcript;
the append-only order is preserved. A future renderer can add call/result links.
