+++
id = "M9-080"
title = "TUI GFM alert labels"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-018"]
docs = ["rendering", "adr-0035"]
+++

## Problem

The web renders `> [!NOTE]` as an alert; the TUI printed `│ [!NOTE]` literally.

## Scope

- Recognize NOTE/TIP/IMPORTANT/WARNING/CAUTION quote labels.
- Render `│ note · …` with the existing quote bar and muted tone.
- Leave ordinary quotes and unknown markers unchanged.
- Unit tests for body/no-body forms.
- No color/variant semantics yet.

## Acceptance

- [x] `> [!NOTE] body` becomes `│ note · body`.
- [x] A bare `> [!WARNING]` becomes `│ warning ·`.
- [x] Ordinary quotes unchanged.
- [x] Existing block tests pass.
- [x] No event-log/parser change.

## Notes

The label is normalized ASCII, not the original marker. A later slice can add
per-variant tone and icons.
