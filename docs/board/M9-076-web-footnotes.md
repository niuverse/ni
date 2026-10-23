+++
id = "M9-076"
title = "Footnotes in web Markdown"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-012"]
docs = ["rendering", "adr-0036"]
+++

## Problem

Footnote references and definitions rendered as literal Markdown in the web
transcript.

## Scope

- Enable `ENABLE_FOOTNOTES` in pulldown-cmark.
- Style references smaller and definitions as a dim top-bordered block.
- Test `[^1]` plus definition output.
- No server/API change.

## Acceptance

- [x] Footnote reference is rendered.
- [x] Definition text appears.
- [x] Styling keeps definitions visually secondary.
- [x] Existing Markdown/tables/code tests pass.
- [x] No unsafe HTML path added.

## Notes

Footnotes are display-only and don't mutate the session. Footnote numbering is
pulldown's, not stored in the event log.
