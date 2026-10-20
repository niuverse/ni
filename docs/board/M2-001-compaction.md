+++
id = "M2-001"
title = "Compaction 与可执行 handoff"
status = "backlog"
milestone = "M2"
module = "context"
size = "L"
priority = "P1"
updated = "2026-10-20"
depends_on = ["M0-009"]
docs = ["adr-0002"]
+++

## 范围

- 上下文到 75% 预算时触发压缩。
- 保留最近 N 轮原文和原始用户请求。
- 生成结构化 handoff：目标、已完成、改动文件、当前状态、下一步、约束。
- 压缩结果落成 `compaction` 事件，resume 时按 `upto` 重建投影。

## 验收

- 压缩后继续任务，不重复已完成步骤。
- 压缩事件可回放，模型上下文与压缩时一致。
- 压缩失败时保留原上下文并提示，不丢历史。
- 长会话 token 成本可见地下降。

## 记录

压缩不是"总结对话"，是给下一个 turn 或下一个 session 写可执行的交接单。
