+++
id = "M9-015"
title = "Generic web context sidebar"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-014"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The web shell was single-column and knew nothing about the embedder's product
state. niuse needed identity, user, goals, plan, memory, and loops visible
without forking the HTML.

## Scope

- `web::Options.brand` and `Options.context` carry product data.
- `GET /api/context` returns the supplied JSON.
- Embedded client sets the brand/title and renders generic `sections`.
- Sidebar is hidden on narrow screens, fixed on wide ones.
- No new store, no niuse-specific HTML fork.

## Acceptance

- [x] `/api/context` returns the supplied JSON over loopback.
- [x] The client fetches it and sets brand/title.
- [x] Sections render as title + lines.
- [x] The sidebar is responsive.
- [x] Missing context falls back cleanly.
- [x] The static client has no remote assets.

## Notes

niuse supplies product sections; niucode just renders them. This keeps the
kernel/product boundary while allowing different embeddings.
