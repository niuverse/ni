+++
id = "adr-0037"
title = "ADR-0037: Native computer use and multimodal input"
kind = "adr"
status = "accepted"
order = 64
summary = "X11-first desktop control with observation-bound actions, four permission tiers, vision attachments, and a strict kernel/product boundary."
+++

## Status

Accepted · 2026-10-23. Linux X11 is the first backend; macOS is the second
milestone. Wayland is explicitly out of scope for the first version.

## Context

Users want the agent to see the desktop and act on it, and to attach images or
scripts to a prompt. A desktop is not a workspace: screenshots contain secrets,
coordinates go stale, and input can bypass shell-tool approvals. This must be a
niucode capability with niuse policy/UX, not a product-layer fork.

## Decision

1. **Platform order.** Linux X11 first, pure Rust `x11rb` + XTEST (no shelling
   out to xdotool/scrot). The observe backend is `niucode-computer-x11`; input
   injection is a separate slice. macOS follows with ScreenCaptureKit, CGEvent, and AX.
   Wayland is not claimed. X11 has no application isolation; these guards are
   product policy, not an OS sandbox.
2. **Permission tiers.** `--computer off|observe|control|autonomous`, default
   `off`. A session must be explicitly armed. `observe` allows observation only;
   `control` requires approval for every input; `autonomous` auto-executes
   ordinary input. In every tier, terminal windows, sensitive/authentication
   windows, system-level shortcuts, and mismatched target windows remain
   approval-gated or rejected.
3. **Actions.** v1: `observe`, `click`, `type`, `key`, `scroll`. No drag,
   clipboard access, app launch, system hotkeys, or macro playback. X11 input
   uses XTEST; ASCII text plus common named keys is the first limit.
4. **Observation binding.** The model-facing `computer` tool schema and typed
   parser live in `niucode-computer`; a session controller keeps an eight-entry
   observation registry and resolves policy from the observed window kind.
   `observe` returns an opaque observation id, window, geometry, screenshot
   hash, scale, and expiry. Input actions carry the
   observation id, window id, and image-relative coordinates. The runtime
   rechecks window identity/geometry and a 60-second expiry; mismatch rejects
   rather than executing an approximate click. Input is revalidated immediately
   before XTEST dispatch.
5. **Lifecycle.** After the session is armed, the model may start/stop
   observation. The agent advertises `computer` only when a runtime is
   configured; `niucode exec --computer` and `niucode tui --computer` connect to
   X11 and arm it. The TUI `/computer` command reports state, re-arms, or stops
   (policy off + disarm). Input still follows policy. The user can stop or downgrade at
   any time; a global emergency stop releases held keys/buttons. Observation is
   never a hidden background capture.
6. **Multimodal.** Provider content gains `text | image` parts. First transport
   is OpenAI-compatible `image_url` with a capability gate: unsupported models
   reject images before sending. User images are content-addressed (0600,
   SHA-256, no base64 in JSONL), recent two are re-sent, older ones become
   `[image omitted: alt]`. User images and the current screenshot share a
   visual budget (max four per request); screenshots are ephemeral by default.
   Resumed sessions and queued follow-ups rebuild history through the same
   projection, so compaction and the recent-image window use one rule. The local web client uploads via a
   token-gated `/api/attach` (16MiB request cap) and sends refs with the prompt.
7. **Text attachments.** TUI `/attach` + `/detach` and user-typed `@path`, CLI
   `--attach`/`--image`, and Web drop-paste; a
   script is text context and is never auto-executed. Per-file 20k chars /
   ~4k tokens; total attachments at most 25% of context; over budget rejects
   instead of silently truncating. Model output never expands `@path`.
8. **Audit.** Log action metadata (window, coordinates, observation hash,
   purpose, approval, result); typed text defaults to length + SHA-256. Full
   text requires an explicit `--computer-log-input`; screenshots require an
   explicit Keep frame.
9. **Memory.** Three layers: append-only JSONL, session context projection, and
   confirmed niuse Markdown memory. Implementation order A (context projection)
   → C (attachment memory) → B (cross-session). No vector database.
10. **Boundary.** niucode owns the backend, tool, permission enforcement, event
    schema, approval, and emergency stop. niuse owns feature enablement,
    defaults, product commands, and prompt policy. The default niuse binary
    stays niucode-free; `computer` is opt-in and no source is forked.
11. **Delivery.** Protocol/permission (this ADR + `niucode-computer`), then
    multimodal, context projection, X11 observe, X11 input, attachment UX, and
    macOS. Each slice is feature-gated and independently testable.

## Consequences

- Linux X11 is testable in Xvfb; macOS runtime testing needs a real Mac and TCC
  grants. Unsigned macOS builds may re-prompt for permissions; signing/helper
  distribution is a later hardening step.
- The protocol crate has no OS dependency, so it compiles on all CI platforms.
- The 60-second observation expiry and strict window hash are deliberate
  friction: a stale click is worse than asking the model to observe again.
- `autonomous` is not a blank cheque; its red lines are enforced in code, not in
  the prompt.

## Non-goals

Wayland, browser-only automation, macro playback, clipboard access, remote
desktop streaming, and unattended background capture.
