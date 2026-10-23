+++
id = "M9-014"
title = "Product system prompt for web"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-013"]
docs = ["rendering", "adr-0036"]
+++

## Problem

`niucode web` used the default coding-agent prompt. niuse's web entry needs its
identity, user profile, memory, goals, plan, loops, and journal.

## Scope

- Make the web module public.
- Add `Options.system_prompt`; the turn loop replaces the default prompt only
  when supplied.
- niuse composes the prompt from its seven stores and passes it.
- Session, transport, SSE, approval, and Markdown stay niucode-owned.

## Acceptance

- [x] `niucode::web` is importable by niuse.
- [x] `Options.system_prompt` overrides the default only when set.
- [x] niuse composes with `compose_with_plan` and its thread.
- [x] No fork of the web server or HTML into niuse.
- [x] niuse builds with niucode non-optional.

## Notes

The web UI still says niucode. Product branding/sidebars are the next niuse
slice.
