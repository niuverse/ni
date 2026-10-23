+++
id = "M9-029"
title = "Collapsible and filterable context sidebar"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-015"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The product sidebar grew to seven sections plus a report. It was a long scroll
with no way to focus on one part.

## Scope

- Render each context section as a native `<details>` with a line count.
- Add a local filter input; hide sections that do not match.
- Keep section content escaped with `textContent`.
- No new API, store, or write path; context is still a page-load snapshot.
- Collapse state is per-render, not persisted.

## Acceptance

- [x] Sections are collapsible and open by default.
- [x] Counts reflect the rendered lines.
- [x] Filter hides non-matching sections.
- [x] Clearing the filter restores all sections.
- [x] Static client assertions cover `ctx-section` and `ctx-filter`.
- [x] Existing sidebar/status/timer tests pass.

## Notes

Persisting collapsed groups or pushing context over SSE is a later slice.
