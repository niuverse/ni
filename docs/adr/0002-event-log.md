+++
id = "adr-0002"
title = "ADR-0002：事件日志是唯一真相"
kind = "adr"
status = "accepted"
order = 21
summary = "内存状态只是 JSONL 日志的 fold；resume、fork、replay 和 compaction 都从日志派生。"
+++

## 状态

Accepted · 2026-10-20

## 背景

Agent 进程会崩溃、被取消、被重启。如果对话只存在内存里，一次崩溃就丢掉全部上下文；如果另外维护一份"session 快照"，它和内存状态迟早不一致。

## 决策

每个 session 是一个 append-only JSONL 文件，一行一个 `Event`。内存状态是日志的 fold：

```text
state = fold(events)
```

- 写入：`SessionLog::append` 序列化一行、flush，不改旧行。
- 读取：`load` 逐行解码；坏行变成 warning，不丢整份历史。
- 恢复：`resume` 从日志重建，而不是从快照恢复。
- 分叉：`fork` 复制某个前缀，生成新 session。
- 压缩：`compaction` 事件记录摘要和 `upto`；只影响投影，不删原始事件。

## 后果

### 正向

- resume / replay / fork 共用一套机制，测试可离线回放。
- 崩溃最多丢最后一行（未 flush），不会出现"内存与磁盘不一致"。
- 审计简单：每个决策、工具调用、usage 都有时间线。
- 压缩是投影变换，可回滚、可对比。

### 负向

- 日志会增长；长会话需要 compaction 和归档策略。
- 事件 schema 演进需要兼容策略（新增字段可默认，删除/改义要迁移）。
- 日志可能包含敏感内容（文件内容、命令输出），需要权限和清理策略。
- 每次 append 都 flush，吞吐低于批量写；对 agent 的写入频率足够。

## 投影规则

1. 原始事件永远保留。
2. 最后一个 `compaction` 之前的消息不进入模型上下文，替换为摘要。
3. `tool_result` 必须和对应 `assistant_message.tool_calls` 成对出现；缺失时投影报错，不猜。
4. reasoning 是否进入上下文由 provider 适配层决定，日志始终保留原文。
5. 投影是纯函数：相同日志 + 相同预算 → 相同上下文。

## 备选方案

- **内存 messages + 退出时快照**：实现简单，但崩溃丢上下文，且快照与内存双真相。
- **数据库（SQLite）**：查询和并发更好，但 v0 引入 schema、迁移和依赖；JSONL 可 grep、可 diff、可直接回放。
- **只存模型可见的投影**：省空间，但丢掉审计和 fork 能力，压缩不可逆。

## 风险与缓解

- 日志过大：按 session 归档，支持 `compact --archive`；看板 Runs 视图只读聚合指标。
- 敏感信息：session 目录默认 `0700`；提供 `niucode session redact` 的后续入口。
- 坏行：加载时 warning + 行号；`--strict` 模式用于 CI。
