+++
id = "adr-0016"
title = "ADR-0016：Provider 重试、无硬 turn 上限与手动 compact"
kind = "adr"
status = "accepted"
order = 43
summary = "只在流尚未产生事件时重试 network/decode/retryable HTTP；上下文溢出强制 compact 后重试；默认 max-steps=0（256 安全上限）；/compact 手动压缩。"
+++

## 状态

Accepted · 2026-10-20

## 背景

一次 4 个 subagent 并行请求遇到 DeepSeek 的 `stream decode error` 和 network
error。旧 loop 在 stream 首次错误时直接失败，且 `Decode` 被标记为不可重试；
24 turn 的默认上限也会在长任务中途截断。上下文预算已有 75% 自动 compaction，
但缺少手动入口和溢出后的恢复路径。

## 决策

1. **请求级重试只在安全点发生**：`Network`、`Decode`、retryable HTTP
   （408/425/429/5xx）最多重试 3 次，指数退避 + `Retry-After` 上限 10s。
   只有当本轮 stream 尚未发出任何 TextDelta/Reasoning/ToolCall/Usage 时才重试；
   已经输出后不重放，避免重复文本和工具调用。
2. **上下文溢出恢复**：provider 返回 `ContextOverflow` 时，强制运行一次
   compaction（不受 75% 阈值限制），成功则用新投影重试本轮；没有收缩则返回原错误。
3. **默认无硬 turn 上限**：`max_steps=0` 表示不设用户级上限，保留 256 的
   安全 turn cap；需要预算时显式 `--max-steps N`。终止仍由模型 final answer
   或用户取消决定。
4. **`/compact`**：TUI 手动触发确定性 handoff；写入 compaction 事件，下一次
   projection 生效。自动 compaction 仍在每 turn 前按 75% 触发。
5. **subagent 共享同一重试策略**，因为重试在 agent loop 的 provider 调用处，
   不在 subagent 之外另写一套。

## 后果

- 4 个并发 subagent 的短暂网络/上游 SSE 抖动可以自愈；永久错误仍逐任务报告。
- 已输出流不会因为重试产生重复工具调用。
- 长任务不会因为 24 turn 被截断；上下文仍由预算 + compaction 控制。
- 无上限时 token 成本由用户取消/模型完成/256 安全 cap 控制，不提供 token 计费硬预算。
- 真实 SSE 中途断开仍会失败（无法安全恢复半条 assistant message）。

## 备选方案

- **所有错误无限重试**：会在鉴权/参数错误上死循环，也会在已有输出后重复工具调用。
- **保留 24 turn 上限**：与长任务冲突；用户需要显式预算而不是默认截断。
- **每次 compaction 调模型总结**：更强但更贵、更慢；当前确定性 handoff 已足够。
