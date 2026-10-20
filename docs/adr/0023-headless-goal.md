+++
id = "adr-0023"
title = "ADR-0023：headless goal loop 与稳定退出码"
kind = "adr"
status = "accepted"
order = 50
summary = "exec --goal 复用 actor/evaluator；met=0、budget=2、impossible=3、cancel=130、evaluator/actor error=1；JSON 输出 goal_verdict/goal_done。"
+++

## 状态

Accepted · 2026-10-20

## 背景

`/goal run` 已在 TUI 可用，但 CI/脚本需要无 TUI 的同一 evaluator loop，并能从退出码判断结果。

## 决策

1. `exec --goal <condition> --max-goal-turns N`：先跑 actor，再调用无工具 evaluator；`not_met` 且预算未耗尽时用投影 history 继续 actor。
2. `--max-goal-turns` 计 actor 轮数，默认 6；0 非法。
3. 退出码：
   - `0` met
   - `1` actor/evaluator/基础设施错误
   - `2` budget exhausted
   - `3` impossible
   - `130` Ctrl-C cancelled
4. JSON（`--json`）逐行输出 `goal_verdict {status,reason}`；终止输出 `goal_done {status:"budget"|"impossible"}` 或普通 `done`。
5. GoalSet/GoalVerdict/GoalCleared 写入 session；replay/resume 可审计。
6. evaluator 仍是无工具、独立 context、30s timeout。

## 后果

- CI 可直接用退出码分流，不需要解析 stdout。
- actor 每轮从事件日志重新投影 history，避免维护第二份对话状态。
- 预算按 actor 轮数，不按 token；token 预算仍由 provider/context 控制。
