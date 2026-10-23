+++
id = "M9-057"
title = "High-contrast and mobile visual fixes"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-056"]
docs = ["adr-0036"]
+++

## Problem

The populated high-contrast render showed a blank Deny button: it inherited the
white border as its background and used white text. On a 390px viewport the
header overflowed (Download/live/event count pushed off-screen) and the only
export path was the hidden Download button.

## Scope

- Deny uses `--panel2` background with `--text`, readable in all themes.
- Below 760px hide secondary header items (`cwd`, `mode`, `download`, event
  count, live status) while keeping brand/model/approval/usage/theme.
- Add local `/export` so mobile can download the Markdown without the header
  button.
- Keep `button.chip` and the primary Send/Allow treatment.
- No server or session change.

## Acceptance

- [x] Deny is readable in high contrast.
- [x] Mobile header no longer overflows.
- [x] `/export` downloads the session.
- [x] Header button and command share one download function.
- [x] Static assertions cover `/export`.
- [x] Existing theme/export tests pass.

## Notes

Mobile screenshots are still manual evidence; no pixel baseline. The sticky
header can overlap content while scrolled; this is expected browser behavior,
not a layout break.
