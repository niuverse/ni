+++
id = "M5-008"
title = "Tool metadata and loop phase seam"
status = "done"
milestone = "M5"
module = "tools"
size = "S"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M4-007"]
docs = ["adr-0027", "architecture"]
+++

## 范围

- 内置工具 risk/effect/read_only metadata。
- `/tools` 审计视图。
- LoopPhase observer 事件。

## 验收

- [x] read/grep/glob low/read；write/edit medium/write；bash/subagent high。
- [x] `/tools` 显示内置与 extension 未知风险。
- [x] LoopPhase 覆盖 project/request/stream/tool/turn/done。
- [x] metadata 单测。

## 记录

- metadata 先观察，不改变 approval/sandbox。
- LoopPhase 只在 AgentEvent 流中，不写 session。
