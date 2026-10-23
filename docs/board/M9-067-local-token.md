+++
id = "M9-067"
title = "Per-run local access token"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-066"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The threat model's main residual risk was real: loopback has no authentication,
so any local process or other local user could call the API.

## Scope

- Generate a 16-byte random token per server run (`/dev/urandom`, time fallback).
- Print the URL with `?token=…`.
- A valid query token on `/` bootstraps an `HttpOnly; SameSite=Strict` cookie.
- All routes (page, API, SSE, export) require that cookie or a valid query token.
- Keep the Host/Origin and CSP guards.
- Tokens are in memory and die with the process.
- Tests cover 401, query token, cookie, and Set-Cookie bootstrap.

## Acceptance

- [x] Unauthenticated API returns 401.
- [x] Query token authorizes API access.
- [x] Root with query token sets the cookie.
- [x] Cookie authorizes the page.
- [x] Existing state/tests without a token remain functional in test helpers.
- [x] No token persistence or logging beyond the startup URL.

## Notes

The token is visible in the browser URL/history and to the same local user; it is
a local isolation measure, not remote auth. A future remote mode still needs a
real auth design.
