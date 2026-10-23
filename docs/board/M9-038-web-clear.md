+++
id = "M9-038"
title = "View-only /clear in the web transcript"
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

Long browser transcripts had no way to clear the view without reloading, which
would also refetch and re-render the whole session.

## Scope

- `/clear` clears the DOM transcript and streaming state only.
- Print `view cleared · session log unchanged` so the semantics are explicit.
- Do not POST, write the session, create a new session, or reset the sidebar.
- Add `/clear` to the command menu and `/help`.
- Keep unknown-slash rejection.

## Acceptance

- [x] `/clear` empties the transcript view.
- [x] It explicitly says the session log is unchanged.
- [x] No API call or session write occurs.
- [x] Streaming state is reset.
- [x] Command menu/help include it.
- [x] Static client assertions cover the note.

## Notes

A true new-session command needs server-side session switching and SSE
reconnect semantics; this slice deliberately avoids implying that.
