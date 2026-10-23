+++
id = "M9-110"
title = "Explicit screenshot Keep frame"
status = "done"
milestone = "M9"
module = "loop"
size = "S"
priority = "P1"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-099"]
docs = ["adr-0037"]
+++

## Problem

Observe screenshots were always ephemeral. ADR-0037 requires an explicit Keep
frame for any persisted screenshot, but no such path existed.

## Scope

- Add `AgentConfig.computer_keep_frames` (default false).
- Add `niucode exec/tui --computer-keep-frames` and niuse's global flag.
- On observe, when enabled, store the PNG in the session attachment store and
  append `frame_kept sha256=…` to the ToolResult.
- Warn at startup that screenshots can contain secrets.
- Keep the image content part and default ephemeral behavior unchanged.
- Test both the default (no store) and opt-in (file + hash) paths.

## Acceptance

- [x] Default observe writes no screenshot file.
- [x] Opt-in writes a content-addressed PNG and reports its hash.
- [x] The model still receives the image part.
- [x] CLI flags/warnings are explicit.
- [x] No automatic retention policy is implied.
- [x] Tests/clippy pass.

## Notes

Kept frames are not attached back into history automatically; they are audit
files. Deletion/retention is the user's store cleanup.
