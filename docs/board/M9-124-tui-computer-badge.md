+++
id = "M9-124"
title = "Persistent computer badge in the TUI footer"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P2"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-100"]
docs = ["adr-0037"]
+++

## Problem

The TUI printed the computer policy once at startup and only showed it again via
`/computer`. After a stop, the footer gave no persistent indication that
computer use was off.

## Scope

- Append `computer <policy> ✓/paused` to the editor footer when a runtime exists.
- Reflect policy off after `/computer stop`.
- Keep the existing cwd/model/permissions/context/goal fields.
- No behavior change when computer is off/unconfigured.
- Existing PTY attachment/Markdown test must remain green.

## Acceptance

- [x] Configured TUI shows policy and armed state.
- [x] Stop changes the badge to off/paused.
- [x] Off mode leaves the footer unchanged.
- [x] PTY regression passes.
- [x] Tests/clippy pass.

## Notes

The footer is width-constrained; this is a short badge, not a full status panel.
