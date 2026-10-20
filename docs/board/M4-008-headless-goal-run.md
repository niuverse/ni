+++
id = "M4-008"
title = "Backlog: headless goal run (exec --goal)"
status = "backlog"
milestone = "M4"
module = "loop"
size = "M"
priority = "P2"
updated = "2026-10-20"
depends_on = ["M4-005"]
docs = ["adr-0014", "exec"]
+++

## 范围

- `niucode exec --goal <condition>` 复用 evaluator loop，不需要 TUI。
- `--json` 输出 `goal_verdict` 事件；met/impossible/budget/error 有明确退出码。
- 预算、超时、取消与 headless cancellation 一致。

## 验收

- [ ] `exec --goal` 可离线 replay 测试。
- [ ] `--json` 包含 verdict 与最终状态。
- [ ] 预算/不可能/取消退出码稳定。
