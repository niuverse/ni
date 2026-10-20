+++
id = "M4-005"
title = "Backlog: /goal run evaluator loop"
status = "backlog"
milestone = "M4"
module = "loop"
size = "M"
priority = "P2"
updated = "2026-10-20"
depends_on = ["M4-001"]
docs = ["adr-0014"]
+++

## 范围

- `/goal run` 在每轮结束后用独立 evaluator 判断 met / not_met / impossible。
- max turns/tokens/time；no-progress 停止；pause/stop；verdict 事件。
- evaluator 无工具权限，只能看会话输出。

## 验收

- [ ] evaluator 不改变 permission profile。
- [ ] 目标达成/不可能/预算耗尽都能停止并记录原因。
- [ ] 非交互模式有明确退出码。
