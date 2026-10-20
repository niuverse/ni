+++
id = "M4-009"
title = "TUI terminal-state hardening and max-steps control"
status = "done"
milestone = "M4"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M4-001", "M4-005"]
docs = ["tui"]
+++

## 范围

- 鼠标捕获只在 slash 菜单打开时启用；菜单关闭/提交/退出后释放，恢复终端原生复制和拖拽。
- `TerminalGuard` 用 Drop 统一恢复 raw mode、bracketed paste、mouse capture 和 cursor。
- TUI 增加 `--max-steps`，默认 24；max-steps 错误给出提升提示，不丢已写文件。
- 鼠标 cursor-position 查询失败时退化为相对 hover，不长时间卡住。

## 验收

- [x] max-steps 后回到 prompt，可继续输入；不残留鼠标捕获。
- [x] 正常提交、slash 菜单、Ctrl-C、panic 路径都能释放终端状态。
- [x] TUI 和 exec 的 max-steps 默认值一致，均可覆盖。
- [x] PTY 回归覆盖菜单、resize、审批、max-steps 和 goal run。

## 记录

- `TerminalGuard::enter/drop`；动态 `sync_mouse_capture`；cursor 查询在菜单打开时、capture 前执行；失败后走相对 hover。
- TUI 启动先发 `DisableMouseCapture` / `DisableBracketedPaste` / `LeaveAlternateScreen` / `Show` / `ResetColor`，清掉上一次崩溃可能残留的终端模式。
- `run_tui --max-steps`（当时默认 24）、`exec --max-steps`（当时默认 24）；M4-012 改为默认 0（256 安全 cap）。
- 复现：`tui --max-steps 1 --replay golden-read.json` → `max steps reached` → 仍出现 prompt；鼠标使能只发生在 `/` 菜单，错误后立即释放。
- 2048 闭环项目在 `niucode-proj/proj_00`：16 个逻辑测试 + 11 个浏览器 e2e（移动、合并、计分、undo、胜负、键盘、触摸、tile containment）。
