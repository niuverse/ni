+++
id = "adr-0015"
title = "ADR-0015：运行时 model 与 permission 选择"
kind = "adr"
status = "accepted"
order = 42
summary = "/model 是当前 provider 内的运行时模型选择；/permissions 映射 ask/review/full-access/ci 到既有 sandbox/network/approval；两者事件化、下一 turn 生效。"
+++

# ADR-0015: Runtime model and permission selection

- Status: accepted
- Date: 2026-10-20
- Supersedes: none
- Related: ADR-0006 (sandbox), ADR-0014 (profiles/plan/goal)

## Context

Interactive sessions are long-lived. Restarting the TUI to try another model
loses conversation state, and users need to see/change the effective sandbox
without editing CLI flags. Pi records a model selection separately from the
model that actually handles each request, and resumes the selection from the
session. niucode needs a small version of that behavior without a full model
catalog or virtual routing.

## Decision

### Model

- `/model` shows the effective model plus optional suggestions.
- `/model <name>` is a runtime override for subsequent turns. It is recorded as
  a `model_change` event and restored by `/resume`; `/clear` carries it forward,
  `/reset` drops it.
- Switches stay on the current provider preset. Cross-provider switching needs
  a new process (`--preset`), because credentials/endpoints are bound at startup.
- The override is applied to `CompletionRequest.model`; the provider's endpoint
  and auth stay unchanged. Each `model_change` is a turn boundary; switching
  may miss the prompt cache.
- Suggestions come from `NIUCODE_MODEL_CHOICES` or a small built-in list. This
  is deliberately not a model registry and has no virtual-model routing in v1.

### Permissions

- `/permissions` shows the current effective sandbox/network/approval and the
  four named profiles.
- `/permissions ask|review|full-access|ci` records a `permission_change` event
  and applies the profile before the next turn.
- Profiles map to existing policy fields; they do not add a second enforcement
  layer:

  | profile | sandbox | network | approval |
  |---|---|---|---|
  | ask | workspace-write | off | ask |
  | review | workspace-write | off | on-failure |
  | full-access | full-access | on | never |
  | ci | read-only | off | never |

- `/plan` remains a runtime mode and narrows the tool allowlist; it is not a
  permission profile. Profile changes take effect on the next turn, never
  mid-turn.
- full-access prints an explicit warning because it disables sandbox/network
  isolation.

## Consequences

- Model and permission choices are event-sourced, resume correctly, and are
  visible in `/status`.
- The provider architecture does not gain a catalog/router abstraction.
- A future model registry or virtual models can replace the suggestion list
  without changing the `model_change` contract.
- Cross-provider switching, per-tool rules, and reviewer-backed `review` remain
  backlog.

## References

- Pi interactive mode: <https://mintlify.wiki/pt-act/pi-mono/guides/interactive-mode>
- Pi virtual models: <https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/virtual-models.md>
