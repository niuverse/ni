+++
id = "M4-001"
title = "CommandRegistry、TUI 自动菜单、/plan 与 /goal"
status = "done"
milestone = "M4"
module = "tui"
size = "L"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M3-002"]
docs = ["adr-0014", "tui"]
+++

## 范围

- 内置 slash 命令收敛到 CommandRegistry，命令元数据同时驱动菜单、帮助和解析。
- TUI 输入层替换 rustyline：输入 `/i` 时在下方自动展示 `/init` 候选，支持 Tab/Enter 接受、↑↓ 选择。
- `/plan` 进入只读 plan mode；生成计划；`/plan off` / `/plan approve` 退出并恢复原 profile。
- `/goal <condition>` 设置会话目标标签；`/goal` 查看；`/goal clear` 清除；持久化到事件日志并随 resume 恢复。
- `/goal run` evaluator loop 不在本卡范围。

## 验收

- [x] 输入 `/i` 自动出现 `/init` 候选；Tab/Enter 可接受，不误执行。
- [x] CommandRegistry 驱动 `/help` 与补全候选，不再有第二份命令表。
- [x] `/plan` 只读；模型写入工具被 allowlist 挡住；退出恢复原 profile。
- [x] `/goal` 设置后进入 system prompt；压缩/resume 后仍存在。
- [x] 非 TTY 退化为逐行输入；PTY 测试覆盖自动菜单、plan、goal。

## 记录

- `crates/niucode-cli/src/slash.rs`：`CommandRegistry` 成为命令元数据来源；`/help` 和菜单共用，不再有第二份命令表。
- `crates/niucode-cli/src/editor.rs`：替换 rustyline 的小型行编辑器。支持自动 slash 菜单、↑↓ 选择、Tab/Enter 接受、历史、光标/Home/End、Ctrl-A/E/U/K/W、Ctrl-C 清空、Ctrl-D 退出、Ctrl-L 清屏、bracketed paste。非 TTY 仍走逐行 fallback。
- `/plan`：`PlanMode { active }` 事件；plan 时 `allowed_tools = read/grep/glob`，system prompt 注入只读计划指令；`/plan off` / `/plan approve` 恢复；`--plan` 可在启动时直接进入。
- `/goal`：`GoalSet` / `GoalCleared` 事件；目标注入 system prompt；`SessionUiState` 从事件恢复，resume 保留。
- `/status` 显示 plan 与 goal；`/init` 在 plan mode 下拒绝。
- 测试：CommandRegistry/parse 单测、editor 菜单单测、SessionUiState 单测、agent plan/goal 集成测试（工具收窄、goal 注入、write 被 allowlist 拒绝）。
- `rustyline` 依赖已移除；TUI 不再有误导性的“假完成”hint。
