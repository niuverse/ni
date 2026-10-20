+++
id = "M3-002"
title = "Subagent 与并行只读探索"
status = "backlog"
milestone = "M3"
module = "loop"
size = "L"
priority = "P2"
updated = "2026-10-20"
depends_on = ["M3-001", "M1-002"]
+++

## 范围

- 子 session 有独立日志，父 session 记录 spawn / result 事件。
- 只读探索可并行；写操作仍由主 session 串行。
- 子代理有独立步数、token 和超时预算。
- 结果返回摘要 + 完整日志路径，不把整个子上下文塞回父上下文。

## 验收

- 两个子代理并行跑互不污染事件日志。
- 子代理超时/失败不影响主 session 继续。
- 父上下文只增加一条结果摘要。
- 能追溯每个结论来自哪个子 session。

## 记录

subagent 不是多 agent 框架，只是"带独立日志的受限子运行"。先证明单 agent 闭环可靠。
