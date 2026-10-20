+++
id = "M4-004"
title = "Backlog: subagent full inheritance"
status = "backlog"
milestone = "M4"
module = "loop"
size = "M"
priority = "P2"
updated = "2026-10-20"
depends_on = ["M4-003"]
docs = ["adr-0014"]
+++

## 范围

- 允许自定义 subagent 继承父 session 的完整工具/插件集合，而不只是 profile。
- 必须显式 opt-in，并受 quota、审计、写锁/隔离约束。

## 触发条件

profile 不够用；需要用户自定义 agent 全量继承；已有更强的 quota/审计。

## 验收

- [ ] full inheritance 只能显式开启，默认关闭。
- [ ] tool view、权限、插件 provenance 可审计。
- [ ] 不允许 subagent 默认获得 full-access。
