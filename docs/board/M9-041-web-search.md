+++
id = "M9-041"
title = "Search the web transcript"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-009"]
docs = ["rendering", "adr-0036"]
+++

## Problem

Long browser sessions had no search. Finding an earlier command or answer meant
scrolling and manual reading.

## Scope

- Add a transcript search input above the log.
- Filter direct log children (messages, cards, approvals) by visible text.
- Show `visible / total` while filtering; clear the count otherwise.
- `Esc` clears the search.
- No backend, index, or session write.

## Acceptance

- [x] Search filters visible messages.
- [x] Count reflects visible/total.
- [x] Escape clears.
- [x] Clearing restores everything.
- [x] Static client assertions cover `filterTranscript`.
- [x] Existing chat/render tests pass.

## Notes

Filtering hides DOM nodes with `hidden`; it never removes evidence or changes the
session. A later slice can mark matches, add next/previous, or search tool output
only.
