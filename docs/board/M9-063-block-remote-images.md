+++
id = "M9-063"
title = "Block remote Markdown images"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-012"]
docs = ["rendering", "adr-0036"]
+++

## Problem

pulldown-cmark emitted `<img src="…">` for Markdown images. A model answer could
therefore make the browser fetch a remote tracking pixel, contradicting the
local/no-remote-assets boundary and leaking that the user opened the page.

## Scope

- Strip every `<img>` after Markdown HTML generation.
- Replace it with escaped alt text: `[image: alt]` or `[image]`.
- Never keep the image URL in the DOM.
- Code fences are unaffected (their content is escaped text).
- No new endpoint or dependency; no images are fetched.

## Acceptance

- [x] Output contains no `<img>`.
- [x] The remote URL is absent from the replaced output.
- [x] Alt text remains visible.
- [x] Code blocks keep literal image syntax.
- [x] Existing Markdown/MathML tests pass.

## Notes

Local data-URI images are also stripped in this slice. A future opt-in could
allow explicitly approved local files without remote fetch.
