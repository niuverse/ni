+++
id = "M9-105"
title = "Computer tool end-to-end under Xvfb"
status = "done"
milestone = "M9"
module = "tools"
size = "S"
priority = "P1"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-099", "M9-097"]
docs = ["adr-0037"]
+++

## Problem

Agent tests used a fake backend and X11 tests used the backend directly. No
single test proved observe image delivery, policy approval, XTEST dispatch, and
the audit event across the real tool loop.

## Scope

- Add a Linux-only integration test that starts Xvfb and maps a window.
- Drive `run_with_history` with a provider that:
  - calls `computer observe`,
  - checks the tool message carries an image,
  - parses the observation id,
  - calls `computer click` at the window center.
- Approve the click through the agent approver.
- Assert a real `ButtonPress` reaches the X11 window and the session log records
  `Event::Approval` for `computer`.
- Skip when Xvfb is unavailable; CI installs it.

## Acceptance

- [x] Observe returns an image part through the agent tool message.
- [x] The parsed observation id is accepted by the click.
- [x] Control policy asks exactly once.
- [x] XTEST click reaches the mapped window.
- [x] Approval is persisted in JSONL.
- [x] Tests/clippy pass.

## Notes

This is a real X server test, but still Xvfb—not a human desktop. Real-desktop
dogfood remains separate evidence.
