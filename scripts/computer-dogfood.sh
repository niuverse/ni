#!/usr/bin/env bash
# Collect a read-only native-computer baseline before exercising approvals.
# Never injects input; the human fills the checklist after the probe.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [ "${1:-}" = "--help" ]; then
    cat <<'USAGE'
usage: scripts/computer-dogfood.sh [OUTPUT.md]

Runs `niucode computer doctor --json`, writes a timestamped evidence template,
and prints the approval/stop checklist. OUTPUT.md is created at 0600. It does
not click, type, arm a model, or write screenshots.
USAGE
    exit 0
fi

binary="${NIUCODE_BIN:-}"
if [ -z "$binary" ]; then
    if [ -x "$root/target/debug/niucode" ]; then
        binary="$root/target/debug/niucode"
    elif command -v niucode >/dev/null 2>&1; then
        binary="$(command -v niucode)"
    else
        echo "error: niucode binary not found; build it or set NIUCODE_BIN" >&2
        exit 1
    fi
fi

if [ "$(uname -s)" = "Linux" ] && [ -z "${DISPLAY:-}" ]; then
    echo "error: DISPLAY is unset; X11 computer use needs a display" >&2
    exit 1
fi

probe="$(mktemp)"
trap 'rm -f "$probe"' EXIT
"$binary" computer doctor --json >"$probe"
if ! python3 -m json.tool "$probe" >/dev/null 2>&1; then
    echo "error: computer doctor did not return JSON" >&2
    cat "$probe" >&2
    exit 1
fi

out="${1:-}"
{
    echo "# Native computer dogfood · $(date -Iseconds)"
    echo
    echo "- host: $(uname -srm)"
    echo "- display: ${DISPLAY:-macOS TCC}"
    echo "- binary: $binary"
    echo
    echo '## Read-only doctor'
    echo
    echo '```json'
    python3 -m json.tool "$probe"
    echo '```'
    echo
    cat <<'CHECKLIST'
## Human acceptance checklist

Mark each only after observing it on the real desktop.

- [ ] `niucode computer doctor --json` reports XTEST/windows/capture (Linux)
      or Screen Recording + Accessibility (macOS).
- [ ] `niucode exec --computer observe "describe the focused window"` returns
      a screenshot observation; no input is injected.
- [ ] `--computer control` click shows the target window, button, and
      image-relative coordinates in the approval.
- [ ] Denying the click leaves the desktop unchanged.
- [ ] Allowing the click reaches the intended window.
- [ ] A `type` approval shows only the character count, never the text.
- [ ] Ctrl-C during/after a turn turns policy off and releases modifiers/buttons.
- [ ] The JSONL has no typed plaintext and no base64 screenshot.
- [ ] macOS: both TCC grants are visible in System Settings; an unsigned rebuild
      re-prompts rather than silently widening access.

## Evidence notes

- approval screenshot:
- window/app used:
- deny result:
- allow result:
- stop result:
- redaction check (`grep` command):
CHECKLIST
} > "${out:-/dev/stdout}"

if [ -n "$out" ]; then
    chmod 600 "$out"
    echo "wrote $out (0600)" >&2
fi
