+++
id = "M9-077"
title = "GFM alerts in web Markdown"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-076"]
docs = ["rendering", "adr-0036"]
+++

## Problem

`> [!NOTE]` and friends rendered as a plain blockquote with literal markers.

## Scope

- Enable pulldown's GFM extension.
- Style note/tip/warning/caution blockquotes with a colored left border and a
  subtle panel background.
- Test `[!NOTE]` produces `markdown-alert-note`.
- Keep existing table/task/footnote behavior.
- No server/API change.

## Acceptance

- [x] `[!NOTE]` gets the alert class.
- [x] Alert variants are visually distinct.
- [x] Plain blockquotes stay muted.
- [x] Existing Markdown/table/task/footnote/code tests pass.
- [x] No raw HTML path is opened.

## Notes

Alert styling is CSS-only; the class names are pulldown's. Unknown alert types
fall back to a normal quote.
