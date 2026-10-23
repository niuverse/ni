+++
id = "M9-113"
title = "Real-desktop and macOS acceptance template"
status = "done"
milestone = "M9"
module = "cli"
size = "S"
priority = "P1"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-112"]
docs = ["computer-acceptance"]
+++

## Problem

CI can prove Xvfb and macOS compilation, but not a human desktop, TCC prompts,
or approval UX. There was no reproducible evidence template.

## Scope

- Add `scripts/computer-dogfood.sh`: read-only doctor JSON + 0600 Markdown
  template; refuses headless Linux; never injects input.
- Add `docs/computer-acceptance.md` with Linux/macOS steps, deny/allow, typed
  redaction, stop, and TCC evidence rules.
- Link from the release notes; no claim that the manual run happened.

## Acceptance

- [x] Script has `--help`, `bash -n`, and no input path.
- [x] Template includes doctor JSON and checklist.
- [x] macOS TCC and fail-closed steps are explicit.
- [x] Redaction/keep-frame checks are explicit.
- [x] Docs/dashboard pass.

## Notes

The checklist is authored, not executed. Real desktop/macOS evidence remains a
human gate.
