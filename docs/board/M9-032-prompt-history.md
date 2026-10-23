+++
id = "M9-032"
title = "Web prompt history"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-031"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The web composer discarded every prompt. Repeating a long request meant
retyping it, unlike the TUI's history.

## Scope

- Persist the last 50 submitted prompts in `localStorage`, newest first.
- Deduplicate; submitting an existing prompt moves it to the front.
- `↑` at the start of a single-line draft recalls older prompts; `↓` moves
  forward and clears at the newest.
- Do not hijack arrows in multi-line drafts.
- Keep history local; no server route or store.

## Acceptance

- [x] History is stored under `niucode.prompt.history`.
- [x] Newest-first, deduplicated, capped at 50.
- [x] Arrow-up/down recall works and `↓` clears at the newest.
- [x] Multi-line drafts keep normal cursor behavior.
- [x] Static client assertions cover history.
- [x] No API or write-path change.

## Notes

Per-workspace histories and a visible history menu are later options.
