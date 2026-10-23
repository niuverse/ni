+++
id = "evidence-web-visual"
title = "Web visual audit"
kind = "evidence"
status = "living"
order = 91
summary = "Headless Chrome screenshots of the empty and populated web shell in light and dark themes, plus the defects they caught."
+++

## Method

The embedded client was copied to a temporary file, `EventSource` was stubbed,
and Chrome (`--headless=new`) rendered it at 1440×1000/1200 with a virtual time
budget. Dark mode was forced through the same `localStorage` key the toggle
writes. The populated fixture exercised:

- session start, user, and assistant messages
- headings, emphasis, inline code, links
- a Markdown table
- a highlighted code block with Copy
- native MathML
- a tool call/result card
- an approval card with a structured diff

## Findings

- The header theme/download/context buttons rendered as blank blue pills because
  the global `button` background beat `.chip`.
- Fixed with `button.chip` panel background, dim text, and accent hover. A static
  assertion now covers the rule.
- Light and dark populated transcripts render with readable table headers, code
  token colors, diff add/remove rows, and approval buttons.
- The empty static page shows placeholder model/cwd/policy chips and "loading
  context…" because no server is present; a live server fills these.
- The approval card keeps the diff, warning slot, and Allow/Deny actions in one
  bounded surface.

## Residual risks

- No pixel-diff regression baseline; screenshots are manual evidence.
- High-contrast mode was reviewed by CSS variables, not a captured screenshot.
- Very long diff/tool outputs scroll inside their card; the page itself stays
  bounded.
- Mobile layout was checked structurally (sidebar hidden below 980px), not by a
  device screenshot.
