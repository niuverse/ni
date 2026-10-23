+++
id = "M9-065"
title = "Href scheme sanitizer"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-064"]
docs = ["rendering", "adr-0036"]
+++

## Problem

Link hardening was a list of string prefixes. Mixed-case schemes (`JaVaScRiPt:`)
and arbitrary schemes (`ftp:`, `blob:`) were not handled, and the code was easy
to bypass.

## Scope

- Parse each generated `<a>` tag and extract its `href`.
- Allow only `http://`, `https://`, `mailto:`, and fragment `#` links.
- Rebuild the anchor with safe `target`/`rel`.
- Replace anything else with `href="#" data-blocked="<scheme>:"`.
- Blocked schemes are lowercased/alphanumeric for the marker.
- No remote fetch or execution.

## Acceptance

- [x] Mixed-case `JaVaScRiPt:` is blocked.
- [x] `ftp:` is blocked.
- [x] `data:` and `file:` remain blocked.
- [x] HTTPS links retain their href and safe attributes.
- [x] Tests cover the safe/unsafe matrix.
- [x] Existing image/Markdown tests pass.

## Notes

Relative paths are intentionally blocked in the transcript; the local API is not
a link surface. A future slice can allow a documented relative-page set.
