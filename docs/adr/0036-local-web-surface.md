+++
id = "adr-0036"
title = "ADR-0036: Local web surface"
kind = "adr"
status = "accepted"
order = 63
summary = "A local-only HTTP surface starts as a read-only session viewer; chat, SSE, approvals, and KaTeX follow without adding a remote control plane."
+++

## Status

Accepted · 2026-10-23. v0 is the read-only `niucode web` viewer.

## Context

The TUI is the reference client, but a browser is better for long Markdown,
tables, and math. A web surface must not become a remote control plane for the
agent or a second source of truth. The event log already has the session shape;
the missing piece is a local transport and a rendering client.

## Decision

1. **Local only.** Bind `127.0.0.1`; no `0.0.0.0`, no remote auth, no cloud
   relay. A future opt-in remote mode needs its own ADR and threat model.
2. **Start read-only.** v0 serves one session plus a live tail: `GET /`,
   `GET /api/session`, `GET /api/health`, `GET /api/events` (chunked SSE), and
   `POST /api/prompt` for one read-only turn at a time (202/409). Runtime deltas
   are broadcast as ephemeral SSE events while the append-only log remains the
   durable source. Browser prompts use `workspace-write` + `ApprovalMode::Ask`;
   an approval event blocks the turn until `POST /api/approval` allows or denies
   it (300s timeout fails closed). Assistant text is rendered to HTML
   server-side with raw HTML escaped and `javascript:` links blocked; the web
   client uses that HTML only for final messages. Math uses native MathML from
   `latex2mathml`, with escaped fallback and no remote assets. An embedder may
   supply `brand` and `context`; a live `context_provider` is called per
   `/api/context` request, feeding a collapsible/filterable sidebar, and
   `/api/status` exposes model/cwd/policy for the header. A finished turn emits
   `context_changed` so the sidebar refetches. Approval previews and tool
   calls/results are rendered as escaped structured cards, not raw JSON. SSE `phase`
   events drive a client-side turn timer; `text_delta` carries accumulated
   rendered HTML, and the durable usage comes from `turn_end`. The composer is a
   multi-line textarea with local prompt history and `/help` `/status` `/report`
   `/clear` commands; code blocks have a Copy button, the session can be
   downloaded as derived Markdown, and a local search filters transcript nodes.
   The viewport follows new output only when already near the bottom, with a
   jump-to-latest control otherwise. A persisted dark/light/high-contrast toggle and
   `prefers-reduced-motion` affect presentation only.
   Read-only remains the default for the CLI.
3. **Same truth.** The viewer reads the append-only session log; it never
   mutates it. Static assets are embedded; the client loads no remote asset.
4. **No build step.** One embedded HTML/CSS/JS file, no npm, no bundler. This
   keeps the local install simple and offline.
5. **Protocol before chat.** The next slice adds SSE events and a prompt POST
   with explicit approval round-trips; every browser action maps to an existing
   runtime event, not a parallel model.
6. **Boundary.** niucode owns the server, protocol, and reference client;
   niuse owns identity/memory/product defaults and may embed or reskin the
   surface. No niucode source is forked into niuse.
7. **Security headers.** `nosniff`, `no-store`, and a strict content type on
   every response. No cookies in v0; no secrets in the HTML.

## Consequences

**Better**

- Rich rendering work can land on the web without destabilizing the TUI.
- The event log is directly inspectable in a browser.
- Offline and loopback-only by construction.

**Worse**

- v0 cannot chat or approve; it is a viewer.
- A browser client duplicates presentation work (Markdown/KaTeX) that the TUI
  approximates in ANSI.
- No authentication means loopback only until a later design.

## Follow-ups

- SSE `/api/events` and POST `/api/prompt`.
- Approval cards with allow/deny POSTs.
- Markdown + KaTeX rendering; keep a no-JS fallback.
- niuse product shell: identity/memory sidebars, dogfood/report views.
