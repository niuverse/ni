+++
id = "M9-017"
title = "Live turn timer and collapsible context"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-016"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The page showed `idle` from a 2.5s poll. It did not show what the agent was
doing or how long the turn had been running, and the sidebar could not be
hidden.

## Scope

- Fold SSE `phase` events into a busy chip (`request`, `stream`, `tool`, ...).
- Start a 200ms timer on the first phase/text delta/approval; stop on `done` or
  `turn_end`.
- Record elapsed seconds in the turn footer.
- Add a header toggle for the context sidebar.
- Keep the polling fallback for status but do not overwrite an active turn.

## Acceptance

- [x] Phase events reach the client over SSE (asserted in the turn test).
- [x] Busy chip shows phase + elapsed.
- [x] `turn_end` clears the timer and records duration.
- [x] Approval switches the busy label to `approval`.
- [x] Sidebar toggle hides/shows context.
- [x] No new write path.

## Notes

The timer is client-side; the durable usage still comes from `turn_end`.
