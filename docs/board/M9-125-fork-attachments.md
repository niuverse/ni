+++
id = "M9-125"
title = "Fork copies referenced attachments"
status = "done"
milestone = "M9"
module = "session"
size = "S"
priority = "P1"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-104"]
docs = ["adr-0037"]
+++

## Problem

Fork copied the event prefix but not the content-addressed attachment store. A
fork in another directory lost recent images even though the events referenced
them.

## Scope

- Collect unique attachment refs in the forked prefix.
- Copy bytes into the destination `attachments/` store at the same SHA-256.
- Skip copying when source and destination stores are the same path.
- Report copied count; missing source files become warnings.
- Test fork → reopen destination store → projection still has the image.
- TUI fork output reports copied attachment count.

## Acceptance

- [x] Forked session loads the referenced image from its own store.
- [x] Unique refs are copied once.
- [x] Same-directory forks do not duplicate files.
- [x] Missing files warn rather than silently producing a broken fork.
- [x] Projection test passes.
- [x] Tests/clippy pass.

## Notes

Text attachments follow the same path. Store cleanup remains manual.
