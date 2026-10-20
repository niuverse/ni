+++
id = "M4-012"
title = "Provider retry, unbounded turns, manual compact"
status = "done"
milestone = "M4"
module = "loop"
size = "M"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M4-005", "M4-009"]
docs = ["adr-0016", "tui", "exec"]
+++

## 范围

- 网络/stream decode/retryable HTTP 在流未产生事件时重试 3 次。
- ContextOverflow 强制 compact 后重试本轮。
- 默认 `max_steps=0`（无硬上限，256 安全 cap）；保留显式 `--max-steps`。
- `/compact` 手动压缩上下文；自动压缩仍按预算 75%。

## 验收

- [x] 空流 Decode/Network 失败会重试并成功。
- [x] 已经输出 partial text 后失败不重放。
- [x] `/compact` 写入 compaction 事件并降低投影。
- [x] 默认 CLI 无 24-turn 截断；`--max-steps` 仍可显式限制。
- [x] subagent 走同一 loop 重试路径。

## 记录

- `MAX_PROVIDER_RETRIES=3`，250ms→500ms→1s（或 Retry-After，上限 10s）。
- `ProviderError::Decode` 加入 retryable；agent 以 `emitted` 保护安全点。
- `maybe_compact(..., force)`；ContextOverflow 强制一次。
- `AgentConfig::new` / CLI 默认 max_steps=0；`UNBOUNDED_SAFETY_TURNS=256`。
- `/compact` 复用 `build_handoff`，不调用模型。
- 测试：resilience 两例、手动 compact PTY、现有全量回归。
