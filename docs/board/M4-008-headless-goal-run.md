+++
id = "M4-008"
title = "Headless goal run (exec --goal)"
status = "done"
milestone = "M4"
module = "loop"
size = "M"
priority = "P2"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M4-005"]
docs = ["adr-0023", "exec"]
+++

## 范围

- `niucode exec --goal <condition> --max-goal-turns N`。
- 复用 actor/evaluator；not_met 继续，met/impossible/budget 终止。
- JSON verdict 与稳定退出码。

## 验收

- [x] replay fixture 覆盖 met / impossible / budget。
- [x] `--json` 输出 `goal_verdict` 与 `goal_done`。
- [x] 退出码 0 / 2 / 3；取消 130；错误 1。
- [x] GoalSet/GoalVerdict/GoalCleared 写 session。

## 记录

- 离线 fixture：met 两轮、impossible、budget 两轮均通过。
- `--max-goal-turns=0` 显式拒绝。
- cancellation 走同一 tokio select，cleanup 后返回 130。
