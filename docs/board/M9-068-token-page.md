+++
id = "M9-068"
title = "Friendly token-required page"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-067"]
docs = ["adr-0036"]
+++

## Problem

Opening the bare loopback URL after token auth landed on a raw JSON 401. A user
who lost the tokenized URL had no explanation in the browser.

## Scope

- Return a small HTML 401 page for `/` without a valid token/cookie.
- Explain that the tokenized URL printed by `niucode web`/`niuse web` is needed.
- Return JSON 401 for API routes (machines keep parsing JSON).
- Test the page and API distinction.
- No change to the auth decision.

## Acceptance

- [x] Root without token shows `Token required`.
- [x] API without token still returns JSON.
- [x] Token/cookie flows unchanged.
- [x] Existing tests pass.

## Notes

The page contains no secret and no script; it is a dead end by design.
