+++
id = "M9-024"
title = "Readable approval previews in the browser"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-011"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The browser approval card stringified `preview` as JSON. A user deciding on a
write had to read raw objects instead of a diff.

## Scope

- Render `PreviewBody::Diff` as per-file headers and numbered add/remove/context
  lines with color.
- Render `Command` as a command block and `Note` as text.
- Escape every part with `textContent`; cap at 200 diff lines.
- Show the warning when present.
- Keep the approval POST semantics unchanged.
- Serialize previews with a stable `kind` tag (tested).

## Acceptance

- [x] Diff files show path, new flag, additions/removals, and line numbers.
- [x] Add/remove lines are visually distinct.
- [x] Command/note previews render without JSON noise.
- [x] Raw preview strings never become HTML.
- [x] Long diffs are truncated in the card.
- [x] Existing approval/allow/deny tests pass.

## Notes

The server still sends the structured preview; rendering is client-side. The TUI
keeps its own renderer for the same structure.
