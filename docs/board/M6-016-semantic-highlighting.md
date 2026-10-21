+++
id = "M6-016"
title = "Shared semantic highlighting and restrained activity animation"
status = "done"
milestone = "M6"
module = "tui"
size = "M"
priority = "P1"
updated = "2026-10-21"
completed = "2026-10-21"
depends_on = ["M6-012", "M6-013"]
docs = ["tui", "tui-upgrade-validation", "adr-0031"]
+++

## 范围

用户补充要求优化 TUI 高亮、动效和 UI。复用 Theme 与 TerminalSession，避免另一套显示框架。
标题、行内代码、代码关键字/数字、diff 增删和结果使用语义高亮；活动区显示真实状态与轻量转轮。

## 验收

- [x] linear 与 inline 使用同一组清洗后样式，文字与字素宽度不因着色改变。
- [x] NO_COLOR / CLICOLOR=0 / TERM=dumb 禁用装饰色与转轮；静态状态仍可读。
- [x] 动画只出现在活动区，每 125ms 至多切一帧；空闲与未变化帧不重复输出。
- [x] 详情显示阅读位置；审批恢复历史位置；Ctrl-A 不被误当作允许操作。
- [x] 原生滚动历史不被折叠重写，退出后模式恢复；没有终端品牌特判。
- [x] 主题单元测试、PTY、独立 tmux 捕获及仓库检查通过；实机字体/IME/闪烁仍由 M6-009 留证。

## 实施记录

2026-10-21：实现与回归通过，详见 `docs/tui-upgrade-validation.md`。真实平台门槛继续由 M6-009 跟踪。
