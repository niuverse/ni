+++
id = "M9-066"
title = "Loopback host and origin guards"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-007"]
docs = ["rendering", "adr-0036"]
+++

## Problem

Loopback-only binding is not an authentication boundary. A malicious page can
send simple cross-origin POSTs to `127.0.0.1`, and DNS rebinding can make a
foreign hostname resolve to the local server. State-changing routes had no
Origin/Host checks.

## Scope

- Reject requests whose `Host` is not `127.0.0.1`, `localhost`, or `[::1]`.
- For POST, if `Origin` is present, require a loopback host; reject foreign or
  `null` origins.
- Add `X-Frame-Options: DENY`, and a CSP with `default-src 'none'`, inline
  script/style only, `img-src 'none'`, `connect-src 'self'`, and
  `frame-ancestors 'none'`.
- Apply headers to HTML/JSON, Markdown downloads, and SSE.
- Tests cover foreign Host and cross-origin POST.

## Acceptance

- [x] Foreign Host → 403.
- [x] Cross-origin POST → 403.
- [x] Missing Origin from non-browser clients still works.
- [x] Loopback Origin from the real client works.
- [x] CSP/frame headers are emitted.
- [x] Existing endpoint/tests pass.

## Notes

This is defense in depth for a local tool, not a multi-user auth model. A future
remote mode still needs explicit tokens and a threat model.
