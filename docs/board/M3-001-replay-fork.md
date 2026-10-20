+++
id = "M3-001"
title = "Replay 与 fork"
status = "backlog"
milestone = "M3"
module = "session"
size = "M"
priority = "P2"
updated = "2026-10-20"
depends_on = ["M2-001"]
docs = ["adr-0002"]
+++

## 范围

- `ni replay <session>`：按事件重建状态并校验投影。
- `ni fork <session> --at <event-id>`：复制前缀，生成新 session。
- 事件 id 稳定，fork 后历史可追溯。

## 验收

- replay 不调用模型，输出与原始 turn 的投影一致。
- fork 后原 session 不被修改。
- 坏尾行 fork 时被截断并提示。
- 事件 id 冲突检测。

## 记录

fork 是 subagent 和"试另一条路"的基础。先有可靠前缀复制，再谈并行。
