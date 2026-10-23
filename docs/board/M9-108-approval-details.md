+++
id = "M9-108"
title = "Computer approval summaries carry safe action details"
status = "done"
milestone = "M9"
module = "loop"
size = "S"
priority = "P1"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-107"]
docs = ["adr-0037"]
+++

## Problem

Approval cards said only "computer click on <window>". The human could not see
coordinates, button, key chord, scroll direction, or typed length, while window
titles/purposes could inject control characters or grow unbounded.

## Scope

- Add action details: click button + coordinates, type character count (never
  text), key chord, scroll direction/count + coordinates.
- Sanitize control characters and truncate app/title/purpose/key for display.
- Keep the full typed text out of approval requests, JSONL, and ToolResults.
- Regression for click/type summaries and a display-cleaning unit test.

## Acceptance

- [x] Click shows button and image-relative coordinates.
- [x] Type shows count only; plaintext absent.
- [x] Key/scroll are explicit.
- [x] Control characters are replaced; long titles are truncated.
- [x] Tests/clippy pass.

## Notes

Coordinates are image-relative, as the model sees them; the backend maps them to
root coordinates. This slice does not add screenshot previews to approvals.
