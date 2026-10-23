+++
id = "M9-064"
title = "Harden Markdown links in the web transcript"
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

Links opened in the same tab, discarding the live session view. Only
`javascript:` URLs were blocked; `data:`, `file:`, and `vbscript:` were not.

## Scope

- Open links in a new tab with `rel="noreferrer noopener"`.
- Block `javascript:`, `data:`, `file:`, and `vbscript:` hrefs, preserving the
  blocked scheme for debugging.
- Test safe external and dangerous links.
- No server/session change.

## Acceptance

- [x] External links carry `target="_blank"` and safe rel.
- [x] Dangerous schemes become `href="#"`.
- [x] Tests assert blocked-scheme markers.
- [x] Raw HTML escaping still applies.
- [x] Existing Markdown/image tests pass.

## Notes

Safe URL allowlisting is prefix-based, not a full URL parser. A future slice can
parse `href` values and allow a documented scheme set.
