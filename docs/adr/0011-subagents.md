+++
id = "adr-0011"
title = "ADR-0011：Subagent 是带独立日志的只读子运行"
kind = "adr"
status = "accepted"
order = 30
summary = "subagent 工具并行跑 1–4 个只读任务；每个任务独立 session 日志，父 session 只记录 spawn/result 和摘要路径。"
+++

## 状态

Accepted · 2026-10-20

## 背景

M3-002 需要并行只读探索，同时保持写操作串行、父子日志可追溯、父上下文不被子 agent 的完整 transcript 污染。直接复用主 agent 循环不够：子运行必须有独立的工具白名单、步数、token、超时和日志。

## 决策

1. **只加一个 `subagent` 工具。** 输入是 `tasks` 数组（1–4 个），每个任务有 `prompt` 和可选的 `max_steps`、`timeout_secs`。不加多个 spawn/wait/kill 工具。
2. **子 agent 只读。** 工具规格和执行白名单都限制为 `read`、`grep`、`glob`；没有 `write`、`edit`、`bash`，也没有 `subagent`，所以不能递归。sandbox 设为 read-only + network off，作为第二道边界。
3. **每个任务独立 session。** 子日志放在父 session 旁的 `<parent-stem>.subagents/` 目录；子 `session_start.id` 等于父 spawn 事件的 id。父 session 追加：
   - `subagent_spawn { id, call_id, task, session, max_steps, timeout_secs, context_budget }`
   - `subagent_result { id, call_id, ok, summary, session, turns, usage }`
4. **并行执行，父 session 串行写。** 任务用 `join_all` 并发跑；父 session 只在 spawn 前和 collect 后写入，不跨 future 共享 `&mut SessionLog`。
5. **父上下文只收到一条 tool_result。** 内容是每个任务的摘要 + 完整子日志路径，不包含子 agent 的 system prompt、tool 调用或完整 transcript。
6. **独立预算。** 默认 `max_steps=6`（上限 12），`timeout_secs=120`（上限 600），上下文预算为 `min(parent_budget, 16k)`；使用同一个 provider/model，但每个任务自己的 `SessionLog` 和 usage。
7. **失败隔离。** 子 agent 超时、provider 错误或 max steps 都变成 `ok=false` 的 result，父 agent 继续。超时/失败后给子日志补 `session_end`；父 session 不受影响。
8. **工具面增加一个是显式取舍。** 并行只读探索无法由现有六个工具完成；子 agent 不增加新的写路径。

## 后果

### 正向

- 并行探索缩短大范围搜索的 wall-clock 时间。
- 父子日志分离，父上下文只增加一条摘要，不会被搜索细节撑爆。
- 每个结论都有子 session 路径，可回到原始 transcript。
- 写操作仍只在主 session，串行边界不变。
- 子 agent 不能递归、不能写、不能跑 shell，边界简单。

### 负向

- provider 会同时收到多个请求；并发限流/配额需要 provider 侧配合。
- 子 agent 只有只读工具，复杂的验证仍需父 agent 自己做。
- 子日志不在 `session list` 的顶层目录里，需要看父事件里的路径。
- 超时直接 drop 子 future，可能留下不完整的子日志（已补 `session_end`，但没有 assistant 结果）。
- 父 tool_result 可能包含多个摘要，仍然是一条消息；摘要长度有上限。

## 备选方案

- **多个工具（spawn/poll/wait）**：灵活，但会把调度和生命周期推给模型，prompt 和状态机都变复杂。
- **子 agent 继承全部工具**：能写文件，但与“写操作仍由主 session 串行”冲突，审批和并发写风险都上升。
- **递归 subagent**：可能无限展开，成本不可控；当前白名单直接排除。
- **把完整子 transcript 塞回父上下文**：信息最全，但违背“父上下文只增加摘要”，也会让缓存和预算失控。
- **独立进程/daemon**：隔离更强，但 M3 仍是单进程内并发，先证明日志和预算边界。

## Revisit 条件

1. 子 agent 需要写操作，必须重新设计写串行、审批和冲突检测。
2. 任务数超过 4 或需要层级子 agent，引入显式调度器/依赖图。
3. provider 并发限流成为瓶颈，需要队列和背压。
4. 需要在 `session list` / 看板中直接浏览子 session，增加索引或 runtime viewer。
