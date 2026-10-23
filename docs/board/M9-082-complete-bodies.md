+++
id = "M9-082"
title = "Read complete web request bodies"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-006"]
docs = ["adr-0036"]
+++

## Problem

The server read at most one 16KiB chunk and parsed the body from it. A prompt
larger than the buffer was silently truncated (and could become invalid JSON).

## Scope

- Read until the header terminator plus `Content-Length` bytes.
- Cap parsing at 1MiB; continue reading across chunks.
- Keep loopback/Origin/token guards unchanged.
- Bound the composer to 100k characters.
- Test a 20k body arriving in one write.
- No endpoint/payload change.

## Acceptance

- [x] Body is read completely across reads.
- [x] `Content-Length` is honored.
- [x] GETs without a body still return immediately.
- [x] Composer caps at 100k.
- [x] Large-body test passes.
- [x] Existing HTTP tests pass.

## Notes

The 1MiB cap is a local safety bound; oversized bodies are not a supported input
and will fail JSON parsing rather than execute a partial prompt.
