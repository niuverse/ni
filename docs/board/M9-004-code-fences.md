+++
id = "M9-004"
title = "Code fence chrome"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-003"]
docs = ["rendering", "adr-0035"]
+++

## Problem

Fenced code was highlighted, but the ``` lines themselves were printed. The
reader saw raw fence markers and no language label.

## Scope

- Hide opening and closing fences from the transcript.
- Show the language once as a dim `▎ rust` label.
- Keep the body highlighted and width-stable.
- Do not confuse a fence with an inline triple-backtick span.
- History/details still read the raw event log.

## Acceptance

- [x] Fences disappear from the terminal output.
- [x] ` ```rust ` produces `▎ rust`.
- [x] An unlabelled fence produces `▎ code`.
- [x] Code body still renders.
- [x] PTY test asserts no ``` and sees the heading + body.

## Notes

The fence label is presentation only. Copying the raw log still yields valid
Markdown, and the details view keeps the original lines.
