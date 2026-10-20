+++
id = "M2-001"
title = "Compaction 与可执行 handoff"
status = "done"
milestone = "M2"
module = "context"
size = "L"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M0-009"]
docs = ["adr-0002", "adr-0008", "exec"]
+++

## 范围

- 上下文到 75% 预算时触发压缩。
- 保留最近 N 轮原文和原始用户请求。
- 生成结构化 handoff：目标、已完成、改动文件、当前状态、下一步、约束。
- 压缩结果落成 `compaction` 事件，resume 时按 `upto` 重建投影。

## 验收

- [x] 压缩后继续任务，不重复已完成步骤。
- [x] 压缩事件可回放，模型上下文与压缩时一致。
- [x] 压缩失败时保留原上下文并提示，不丢历史。
- [x] 长会话 token 成本可见地下降。

## 记录

实现位于 `crates/niucode-agent/src/context.rs`（Context 属于 Engine，不放进 core，避免 core 依赖 provider 的 `ChatMessage`）。

- `estimate_context`：ASCII 约 4 字符/token，非 ASCII 1 字符/token，附带 tool schema 与消息固定开销。默认预算 64k，75% 触发；`exec` / `tui` 暴露 `--context-budget` 和 `--keep-recent-turns`。
- `build_handoff`：从日志生成确定性交接单，包含 Goal、Previous request、Work completed、Changed files、Last assistant state、Recent failures、Constraints、Next step。没有额外模型调用。
- cutoff 选择：多轮保留最近 N 个 user turn 原文；单个长 turn 回退到该 turn 内安全的 assistant 边界，避免切开 tool call 和 tool result。
- `compaction.upto` 定义为绝对事件下标，摘要替换 `events[..upto]`。`project_history` 在 cutoff 处插入 system handoff，只使用最后一个 compaction，忽略旧的和非法 cutoff。
- 压缩前先构造候选投影并估算 after；只有 after < before 才追加 `compaction` 事件，事件记录 `before_tokens` / `after_tokens`。失败（无安全 cutoff、不收缩、写入失败）保留原上下文，通过 `AgentEvent::Warning` 提示，历史不受影响。
- 成功压缩时 loop 用同一份 `project_history` 重建 `messages`，TUI / exec 显示 `compacted: N events · ~before → ~after tokens`。

测试覆盖：token 估算、75% 阈值、cutoff 选择、handoff 内容、最后一个 compaction 生效、非法/空摘要 compaction 忽略、单 turn 回退、压缩失败保留上下文、压缩事件的 resume 投影与 provider 请求一致。另用 PTY 跑过 TUI 两轮 compaction，并用 `exec --resume` 验证 headless 压缩通知、事件字段与 resumed 投影；`session show` 会打印 compaction 的 `upto` 和 token 估算。

决策见 ADR-0008。当前 handoff 是确定性模板，token 估算不是精确 tokenizer；两处都是明确的后续替换点。
