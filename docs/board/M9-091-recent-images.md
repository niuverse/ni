+++
id = "M9-091"
title = "Recent-image context projection"
status = "done"
milestone = "M9"
module = "context"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-090"]
docs = ["adr-0037"]
+++

## Problem

Attachment turns worked once, but resuming a session projected only text. A
follow-up question about an earlier image would fail, and sending every image on
every turn would blow the visual budget.

## Scope

- Add `project_history_with_attachments`.
- Select only the most recent two unique user images after compaction.
- Older images become `[image omitted: alt]`.
- Text attachments resolve from the store; missing files become explicit
  markers, not silent drops.
- `run_with_attachments` rebuilds history through this projection.
- Test three images + a text attachment: first omitted, last two sent.

## Acceptance

- [x] Only the last two unique images become provider parts.
- [x] Older images carry an omission marker.
- [x] Text attachments remain in the resumed context.
- [x] Missing text files do not fail the projection.
- [x] Compaction cutoff is respected.
- [x] Existing projection/turn tests pass.

## Notes

The chosen image budget is fixed at two in the agent entry; a later config knob
can expose it. No event data is rewritten.
