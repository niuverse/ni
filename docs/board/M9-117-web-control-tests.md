+++
id = "M9-117"
title = "Configured Web arm/stop tests"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P2"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-106", "M9-116"]
docs = ["adr-0037"]
+++

## Problem

Web computer control had only an unconfigured 409 test. The actual stop path
(policy off + disarm) and the thumbnail UI marker were unverified.

## Scope

- Add a fake backend and a configured `ComputerRuntime` to a web test.
- POST `/api/computer {action:"stop"}` and assert HTTP 200, JSON policy off,
  armed false, and the runtime state afterward.
- Add a static assertion for the `approval-shot` element.
- Keep the unconfigured 409 test.

## Acceptance

- [x] Configured stop returns 200.
- [x] Runtime policy is off and disarmed.
- [x] Unconfigured servers still return 409.
- [x] Approval thumbnail markup is present.
- [x] Tests/clippy pass.

## Notes

The browser control remains arm/stop only; policy selection is startup-scoped.
