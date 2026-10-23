+++
id = "M9-085"
title = "Final visual audit"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-075", "M9-077", "M9-081"]
docs = ["rendering"]
+++

## Problem

Features landed incrementally; the final composition of Markdown, approvals,
copy, and mobile had not been checked together.

## Scope

- Render a populated transcript at 1360×1000 in light/dark/high contrast and at
  390×844.
- Include tables, code, math, tasks, alerts, footnotes, tool cards, approval
  diff, countdown, and copy.
- Record observations and non-findings.
- No code change unless a blocking defect appears.

## Acceptance

- [x] All four renders compose without overflow outside intended scroll areas.
- [x] No raw Markdown markers leak.
- [x] High-contrast Deny remains readable.
- [x] Mobile composer and Latest affordance work.
- [x] Audit evidence committed.

## Notes

The static harness does not populate `/api/context`; sidebar loading text is
expected. Smooth-scroll timing in a screenshot is not a product failure.
