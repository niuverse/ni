+++
id = "M9-087"
title = "OpenAI-compatible vision content parts"
status = "done"
milestone = "M9"
module = "providers"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-086"]
docs = ["adr-0037"]
+++

## Problem

`ChatMessage.content` was text-only, so image attachments had no provider path
and a text-only model would receive an opaque API failure.

## Scope

- Add untagged `MessageContent::Text | Parts` and `ContentPart::Text | ImageUrl`.
- Preserve existing text wire behavior.
- Add `ChatMessage::user_with_images` and `has_images`.
- Add catalog `supports_images` plus `model_supports_images`.
- Add `OpenAiConfig.supports_images` (custom endpoints can opt in) and reject
  images before sending when unsupported.
- Reserve 1024 tokens per image in the projection estimator.
- No attachment storage or UI yet.

## Acceptance

- [x] Text-only messages still serialize as a string.
- [x] Image messages serialize as an OpenAI content array.
- [x] DeepSeek preset is rejected for images before HTTP.
- [x] OpenAI vision presets are enabled by the catalog.
- [x] Custom endpoints can opt in explicitly.
- [x] Token estimation reserves image budget.
- [x] Provider/agent tests pass.

## Notes

The attachment store and budget enforcement are the next slice. This card only
opens the wire/content path and capability gate.
