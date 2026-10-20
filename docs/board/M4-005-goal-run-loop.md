+++
id = "M4-005"
title = "Goal run evaluator loop (TUI v1)"
status = "done"
milestone = "M4"
module = "loop"
size = "M"
priority = "P2"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M4-001"]
docs = ["adr-0014", "tui"]
+++

## 范围

- `/goal <condition>` 设置后立即启动；`/goal run` 可重新启动；最多 6 轮。
- 每轮结束用独立 evaluator 判断 met / not_met / impossible；evaluator 无工具、独立请求。
- verdict 写入 `goal_verdict` 事件；met 自动清除 goal，impossible/预算耗尽停止。
- `/goal stop` 停止；`/goal clear` 停止并清除；`/goal` 显示 run 状态。

## 验收

- [x] evaluator 不改变 permission profile，也不带工具。
- [x] 目标达成/不可能/预算耗尽都能停止并记录原因（TUI）。
- [x] `/goal <condition>` 自动续跑，PTY 端到端验证 not_met → met → clear。
- [x] evaluator 返回未知/缺失 status 时停止，不继续烧 token。

## 记录

- 新增 `crates/niucode-cli/src/goal.rs`：evaluator prompt、30s timeout、JSON verdict 解析（支持 fenced JSON）。
- `GoalAction::Run/Stop`；`SessionUiState.goal_run/goal_turns/pending_prompt`；Set 分支同时启动 run。
- 主循环支持 queued auto prompt；成功后调用 evaluator；`GoalVerdict` 事件；met 追加 `GoalCleared`。
- 预算 `MAX_GOAL_TURNS=6`；plan mode 下拒绝 `/goal run`。
- 测试：verdict 解析（met/not_met/impossible/非法）、`/goal run` 解析、PTY 端到端两轮 evaluator。
- headless `exec --goal` 与 JSON 退出码在 M4-008。
