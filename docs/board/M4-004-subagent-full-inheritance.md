+++
id = "M4-004"
title = "Subagent full inheritance (explicit opt-in)"
status = "done"
milestone = "M4"
module = "loop"
size = "M"
priority = "P2"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M4-003"]
docs = ["adr-0014", "adr-0011", "exec"]
+++

## 范围

- 允许自定义 subagent 继承父 session 的完整工具/插件集合，而不只是 profile。
- 必须显式 `--allow-subagent-inheritance`；默认关闭。
- 继承时仍剥离 `subagent` 工具，禁止嵌套；保留父 sandbox/approval/write lock/write_scope。
- 不改变默认 full-access 边界；inherit 的 access 来自父配置。

## 验收

- [x] full inheritance 只能显式开启，默认关闭。
- [x] inherit 的 tool view 记录在 spawn profile，且不包含 `subagent`。
- [x] 继承父 sandbox/approval/write lock/plan mode；不默认放宽 full-access。
- [x] 无 flag 时返回明确错误，不启动子运行。

## 记录

- `subagent` schema 增加 `profile: "inherit"`。
- `AgentConfig.allow_subagent_inheritance`（默认 false）；CLI `--allow-subagent-inheritance` 同时覆盖 `exec` 与 `tui`。
- inherit：复制父 allowed_tools（或从 built-ins + extension specs 构造全集），再移除 `subagent`；共享父 sandbox/approval/plan_mode/write_lock/write_scope。
- spawn 事件 profile 记录 `inherit`；无 flag 时 tool result 报错，父 agent 继续。
- 测试：默认拒绝 + 明确错误、显式允许后 tool view 包含 write/bash 但不含 subagent。
- 这是 C 的 v1：显式、可审计、不可递归；不等于默认给 subagent full-access。
