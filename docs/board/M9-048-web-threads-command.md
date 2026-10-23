+++
id = "M9-048"
title = "/threads context command"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-035"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The generic context sidebar can contain an archived-thread section, but there
was no quick way to focus it from the composer.

## Scope

- Add local `/threads` command.
- Clear/set the context filter to `threads`, reload context, report in status.
- Add it to `/help` and the completion menu.
- Keep unknown-slash rejection and no model turn.
- No server route or session switching.

## Acceptance

- [x] `/threads` is local and never calls `/api/prompt`.
- [x] It filters the context sidebar through the existing path.
- [x] Help/menu include it.
- [x] Static client assertion covers it.
- [x] Existing commands tests pass.

## Notes

This only focuses an embedder-provided section. Real switching between archived
sessions needs a separate path/session protocol.
