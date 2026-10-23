+++
id = "computer-acceptance"
title = "Native computer manual acceptance"
kind = "guide"
status = "living"
order = 73
summary = "Real-desktop and macOS acceptance: read-only doctor, observe, deny/allow, typed-text redaction, emergency stop, and TCC evidence."
+++

## Purpose

Xvfb proves the protocol and XTEST path, not a human desktop. Run this checklist
on a real Linux X11 session and on a permission-granted Mac before calling the
computer feature usable.

Collect the read-only baseline first:

```sh
scripts/computer-dogfood.sh docs/evidence/computer-dogfood-$(date +%F).md
niucode computer doctor --json
```

The script never clicks, types, arms a model, or writes screenshots. It only
runs the doctor and writes a 0600 Markdown template.

## Linux X11

1. `niucode computer doctor --json` must show `xtest: true`, at least one
   window, and an image size. A missing DISPLAY is an environment problem, not
   a policy problem.
2. Use a vision-capable model, then:

   ```sh
   niucode exec --computer observe "describe the focused window"
   ```

   The model must receive a screenshot; no input occurs.
3. Run `--computer control` and ask for one click on a harmless target.
   - The approval must name the window, button, and image-relative coordinates.
   - Deny once: the desktop must not change.
   - Allow once: the intended target must receive the click.
4. Ask for `type` into a scratch editor. The approval must show only a character
   count. After the turn, grep the JSONL:

   ```sh
   grep -R "the-typed-text" ~/.niucode/sessions
   grep -R "data:image/png;base64" ~/.niucode/sessions
   ```

   Both must be empty unless `--computer-log-input` / `--computer-keep-frames`
   was explicitly used.
5. Press Ctrl-C while a turn is running. Confirm `policy=off`, `armed=false`
   from `/computer` (TUI/REPL) or the web status, and that no modifier/button is
   stuck.

## macOS

1. Grant Screen Recording and Accessibility to the actual binary in System
   Settings. Unsigned rebuilds may re-prompt; that is expected.
2. `niucode computer doctor --json` must show both permissions and a non-null
   image when Screen Recording is granted.
3. Repeat the observe/deny/allow/type/stop checks. Retina coordinates are
   mapped from the captured image; verify a click lands on the intended target,
   including a window moved after observation (the backend must reject it).
4. Record the TCC prompts and any re-prompt after rebuild. A missing grant is a
   fail-closed result, never a silent fallback.

## Evidence

Attach:

- doctor JSON;
- the session path and the relevant approval/tool events (no typed plaintext);
- deny and allow observations;
- stop state;
- macOS TCC screenshots (manual, never committed with secrets).

Do not paste real secrets or full screenshots into the repository.
