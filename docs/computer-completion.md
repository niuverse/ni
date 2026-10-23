+++
id = "computer-completion"
title = "Native computer and multimodal completion audit"
kind = "evidence"
status = "living"
order = 75
summary = "Requirement-by-requirement evidence for the 100-round computer/multimodal objective, with the three remaining external gates called out."
+++

## Scope

This audit maps the objective to code and executable evidence. It does not
claim real-desktop or macOS runtime acceptance.

## Matrix

| Requirement | Evidence | Status |
|---|---|---|
| Linux X11 window enumeration + PNG capture | `niucode-computer-x11`, Xvfb tests, CI | done |
| Observation-bound click/type/key/scroll | X11 XTEST + Xvfb end-to-end, identity/geometry rechecks | done |
| Four tiers `off/observe/control/autonomous` | protocol/session tests, CLI/TUI/REPL/web flags | done |
| Explicit arm + `/computer stop|arm` | TUI/REPL tests, web configured-stop test | done |
| Approvals for control input | agent fake-backend + Xvfb approval test | done |
| Terminal/sensitive/window-mismatch red lines | policy tests, moved-window Xvfb test | done |
| System-shortcut red lines | unit tests (Super, Alt+Tab/Escape/F4, Ctrl+Alt+Delete/Backspace/F1-F12) | done |
| Sensitive-window observation gate | deny/allow tests, no image on deny | done |
| Emergency stop + input release | runtime test, X11 release, TUI/REPL/web paths | done |
| Typed-text privacy | JSONL redaction, opt-in test, structured audit has no text | done |
| Screenshot privacy | ephemeral default, Keep-frame opt-in, bounded approval thumbnail | done |
| Approval target context | coords/keys/type length, in-memory web thumbnail | done |
| Per-action DoS bounds | 10k text, 64 key, 50 scroll, tests | done |
| macOS backend | `niucode-computer-macos` compiles/clippy/tests on macOS CI | code done; runtime gate open |
| Vision image parts + capability gate | provider tests, agent image-part test | done |
| Attachments: CLI/TUI/Web, budgets, content-addressed store | tests across CLI/TUI/web/agent | done |
| A: context projection | projection/compaction tests | done |
| C: attachment memory across reopen | `attachment_memory` test | done |
| C: fork copies attachments | `fork_copies_referenced_attachments` | done |
| B: cross-session Markdown memory | reopened-store two-session test | done |
| Kernel/product boundary | computer crates + niuse flags; default TUI remains opt-in | done |
| Release readiness | 0.1.8/0.1.14, preflight `--skip-package`, runbook | prepared; publish gate open |
| Real Linux desktop | `docs/computer-acceptance.md` checklist | external gate open |
| macOS TCC/runtime | acceptance checklist; only compile/unit evidence | external gate open |

## External gates

1. **Publish authorization.** Irreversible crates.io upload; no `--execute` has
   run. Full package preflight must follow the kernel publish order.
2. **Real Linux X11 acceptance.** Run `scripts/computer-dogfood.sh` on a human
   desktop and complete `docs/computer-acceptance.md`.
3. **macOS acceptance.** Grant Screen Recording + Accessibility on a real Mac,
   run `niucode computer doctor`, then observe/approve/stop checks.

## What is intentionally not claimed

- Wayland, drag, clipboard, app launch, macro playback, background capture.
- TUI image preview (terminal protocol dependent).
- Unsigned macOS builds retaining TCC across rebuilds.
- Correctness of the desktop state after an action (evidence is descriptive).

## Verify

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
./scripts/release-preflight.sh --skip-package   # from niuse
```
