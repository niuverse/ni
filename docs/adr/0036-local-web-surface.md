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
   relay. Each run prints a random token URL; the token bootstraps an HttpOnly
   SameSite cookie required by page/API/SSE/export routes. Reject non-loopback
   `Host` headers and, for POST, foreign/`null` `Origin`s; serve a strict CSP and
   `X-Frame-Options: DENY`. A future opt-in remote mode needs its own ADR and
   threat model.
2. **Start read-only.** v0 serves one session plus a live tail: `GET /`,
   `GET /api/session`, `GET /api/health`, `GET /api/events` (chunked SSE), and
   `POST /api/prompt` for one read-only turn at a time (202/409). Runtime deltas
   are broadcast as ephemeral SSE events while the append-only log remains the
   durable source. Browser prompts use `workspace-write` + `ApprovalMode::Ask`;
   an approval event blocks the turn until `POST /api/approval` allows, allows
   for the tool session, or denies it (300s timeout fails closed); the card shows
   a matching countdown, and the tool allowlist is cleared on new/switch. Assistant text is rendered to HTML
   server-side with raw HTML escaped, `javascript:`/`data:`/`file:`/`vbscript:`
   links blocked by a parsed allowlist, external links opened in a safe new tab,
   and remote Markdown images stripped to alt text so the page never fetches
   third-party resources; the web client uses that HTML only for final messages. Math uses native MathML from
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
   jump-to-latest control otherwise. A persisted dark/light/high-contrast toggle,
   first-run OS color/contrast preferences, and `prefers-reduced-motion` affect
   presentation only. Controls have accessible names; the transcript is a polite
   log region. Sections tagged `threads` switch archived sessions through an
   embedder allowlist, and successful new/switch operations broadcast
   `session_switched`. A Rust test checks DOM IDs and `getElementById`
   references in the embedded client; an optional headless-Chrome test loads the
   file with a stubbed SSE and asserts the log role, brand, and composer. A
   visual audit caught a header chip/button specificity bug and a blank Deny
   button in high contrast, now covered by static assertions.
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
7. **Security headers.** `nosniff`, `no-store`, a strict CSP, and a strict
   content type on every response. The only cookie is the HttpOnly SameSite
   local token cookie; the random token appears in the startup URL, not in the
   static HTML.

## Consequences

**Better**

- Rich rendering work can land on the web without destabilizing the TUI.
- The event log is directly inspectable in a browser.
- Offline and loopback-only by construction.

**Worse**

- v0 cannot chat or approve; it is a viewer.
- A browser client duplicates presentation work (Markdown/KaTeX) that the TUI
  approximates in ANSI.
- The per-run token is local isolation, not multi-user/remote authentication;
  same-user processes can still access the URL/cookie store.

## Follow-ups

- SSE `/api/events` and POST `/api/prompt`.
- Approval cards with allow/deny POSTs.
- Markdown + KaTeX rendering; keep a no-JS fallback.
- niuse product shell: identity/memory sidebars, dogfood/report views.
