# Generation-limit fix: separate follow-up

This directory does **not** replace or amend `../2026-10-21-v1`.
Its 20 original files are byte-identical to the pre-fix snapshot. The original
96 scored attempts and both failures remain the baseline.

## Deterministic before/after comparison

`reconstructed-length.json` contains the final response's text/reasoning deltas
and usage from the failed v1 niucode enhanced Polyglot transpose run.
**The `length` finish reason is injected:** v1 did not persist the wire reason.
This is a regression reconstruction, not an exact recording of the original
network stream. The source console SHA-256 is in `replay-comparison.json`.

Both binaries received the same fixture and a fresh original transpose seed:

| Binary | Exit | Last JSON event | Independent upstream tests |
|---|---:|---|---|
| Original v1 | 0 | done | 0/12 |
| Fixed | 1 | error: finish_reason=length | 0/12 |

The source remains a stub in both cases. This demonstrates truthful failure
reporting, not improved implementation from replaying an incomplete response.
`before/` and `after/` preserve stdout, stderr, session log, workspace and grade.

To repeat with either binary and a fresh workspace/session directory:

```sh
cp -R benchmarks/anchors/polyglot-py-transpose/seed /tmp/transpose-replay-work
niucode exec 'Implement transpose as specified by the original task' \
  --json --replay benchmarks/evidence/2026-10-21-truncation-fix/reconstructed-length.json \
  --cwd /tmp/transpose-replay-work --session-dir /tmp/transpose-replay-sessions \
  --approval never --sandbox full-access --network on
```

Use the original `task.md` content for the exact task prompt; replay does not
consult a live model. Binary hashes are recorded in `replay-comparison.json`.

## Independent live retry

Run `followup-truncation-polyglot-py-transpose-niucode-enhanced-0` used a new image,
new seed, unchanged task/grader, the same DeepSeek high/8192 configuration and
the enhanced workflow. The only adapter/runner edits remove unused imports.
The previous image/tag was not overwritten. `followup-plan.json` freezes the
case/image/runner hashes; `toolchains.json` pins Rust source hashes and parent
image. `niucode-source.tar.gz` and `runner/` preserve the corresponding sources.
`Dockerfile` documents the three-file overlay on the original image.

Result: **12/12 tests passed**, 12 responses, 43.814 seconds, exit 0.
The response reasons are now recorded in the session. This one fresh attempt
did not hit the output limit; it is not causal evidence of higher solve rate
and is not pooled into the v1 score.

The gateway reused the original append-only budget journal. `cost.json` records
12 new requests, CNY 0.263596 upper-bound additional cost, and CNY 14.482944
cumulative upper-bound cost under the shared CNY 300 cap. The stopped gateway's
complete journal is `billing-cumulative.jsonl`; its earlier records include v1
and development attempts, not additional runs in this directory. The original
archived v1 journal remains unchanged.

## Regression coverage and cleanup

- Agent: reasoning-only length, partial answer with truncated/complete tool
  arguments, no dispatch/retry/success, durable response/usage/reason.
- Interactive runtime: no TaskDone on length, error persisted.
- CLI: nonzero exit and error, no done, usage preserved.
- Goal evaluator: even syntactically complete JSON cannot produce met on length.
- Core: old TurnEnd logs decode without the new optional field.
- Existing normal completion/tool dispatch/cancellation/stream-error tests pass.

Removed the unused styled-prompt argument and its sole caller's formatting
branch, plus two unused Python imports: net 11 production lines removed in the
cleanup paths. No fixture, lockfile, cancellation or budget protection removed.
Verification: 195 Rust tests, five Python tests, fmt, clippy and dashboard check.
`checks.json` and `SHA256SUMS.json` capture validation and artifact integrity.
