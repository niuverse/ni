+++
id = "M4-010"
title = "Session lifecycle commands: reload/reset/resume"
status = "done"
milestone = "M4"
module = "tui"
size = "M"
priority = "P2"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M4-001", "M4-009"]
docs = ["tui"]
+++

## 范围

- `/reload`：重读 AGENTS.md，显示 sources/digest，下一次 turn 生效。
- `/clear`：新对话，保留 plan/goal，停止 goal run，保留输入历史。
- `/reset`：新对话 + 清 plan/goal/pending/输入历史 + reload instructions。
- `/resume [path|id]`：列最近 session 或切换到指定 session，恢复 history/plan/goal，停止 goal run。
- `/resume` 不改 cwd/model；不一致时警告。

## 验收

- [x] 三条命令进入 CommandRegistry、自动补全和 `/help`。
- [x] `/reset` 与 `/clear` 的差异可观察、可测试。
- [x] `/resume` 能列出并切换 session，恢复投影状态。
- [x] PTY 覆盖 reload → plan → goal → clear → reset → resume；无 panic。

## 记录

- `SessionUiState.clear_history` 把 `/reset` 的输入历史清空请求传回行编辑器。
- `/clear` 将保留的 plan/goal 重新写入新 session 的 `PlanMode` / `GoalSet` 事件。
- `recent_sessions` 按 mtime 取最近 8 个；`resolve_session_path` 支持相对当前目录、session 目录和省略 `.jsonl`。
- `/reload` 只处理 AGENTS.md；extension host 仍在 session 生命周期内，重启 TUI 才重新挂载。
