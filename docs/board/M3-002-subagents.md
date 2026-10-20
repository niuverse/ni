+++
id = "M3-002"
title = "Subagent 与并行只读探索"
status = "done"
milestone = "M3"
module = "loop"
size = "L"
priority = "P2"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M3-001", "M1-002"]
docs = ["adr-0011", "exec"]
+++

## 范围

- 子 session 有独立日志，父 session 记录 spawn / result 事件。
- 只读探索可并行；写操作仍由主 session 串行。
- 子代理有独立步数、token 和超时预算。
- 结果返回摘要 + 完整日志路径，不把整个子上下文塞回父上下文。

## 验收

- [x] 两个子代理并行跑互不污染事件日志。
- [x] 子代理超时/失败不影响主 session 继续。
- [x] 父上下文只增加一条结果摘要。
- [x] 能追溯每个结论来自哪个子 session。

## 记录

实现位于 `crates/niucode-agent/src/subagent.rs`，工具 schema 在 `niucode-tools`，loop 在工具派发前拦截 `subagent`。

- `subagent` 接受 1–4 个任务；每个任务可指定 `prompt`、`max_steps`（默认 6，上限 12）、`timeout_secs`（默认 120，上限 600）。
- 子 agent 白名单只有 `read` / `grep` / `glob`；sandbox 为 read-only + network off，没有 write/edit/bash，也没有 `subagent`，不能递归。
- 每个任务一个独立 `SessionLog`，放在父 session 旁的 `<parent-stem>.subagents/`；子 `session_start.id` 等于 spawn id。
- 父 session 追加 `subagent_spawn` / `subagent_result` 事件，包含 id、call_id、任务、子日志路径、步数/超时/context budget、结果、turns 和 usage。
- 任务用 `futures_util::future::join_all` 并发跑；父 session 只在 spawn 前和 collect 后写入，没有跨 future 的 `&mut SessionLog`。
- 父模型只收到一条 tool_result：每个任务的摘要 + 子日志路径；完整 transcript 留在子日志里。
- 子代理上下文预算为 `min(parent_budget, 16k)`；超时/失败会补 `session_end` 并返回 `ok=false`，父 agent 继续。

测试覆盖：并行 barrier（顺序执行会超时）、两个子日志互不污染、父请求只有一条 tool_result、子请求只看到三个只读工具、超时隔离、provider 错误隔离、父 session 可继续。决策见 ADR-0011。

M3 里程碑已随 replay/fork、subagent 与 `exec --json` 全部收口。
