+++
id = "M9-006"
title = "Streaming caret and tool timer"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-005"]
docs = ["rendering", "adr-0035"]
+++

## Problem

The activity footer had a braille spinner but no signal that text was actively
streaming, and a running tool showed only the whole-turn elapsed time. Both
made a slow turn look stalled.

## Scope

- Add a blinking `▍`/`▏` caret while assistant text is streaming.
- Disable the caret with the existing animation-off path.
- Show the running tool with its own elapsed seconds in the footer.
- Clear the tool timer on result and turn end.

## Acceptance

- [x] Responding footer alternates caret frames with animation on.
- [x] Animation off keeps a stable footer without a caret.
- [x] Running tool footer shows `name · N.Ns`.
- [x] Timer clears on tool result and turn end.
- [x] No transcript or event-log writes are added.

## Notes

This is footer-only motion; history is not reflowed. A future slice can add a
completion transition and a cancellable progress card.
