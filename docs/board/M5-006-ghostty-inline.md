+++
id = "M5-006"
title = "Stabilize inline TUI on Ghostty"
status = "backlog"
milestone = "M5"
module = "tui"
size = "M"
priority = "P1"
updated = "2026-10-20"
depends_on = ["M5-003", "M5-005"]
docs = ["adr-0022", "adr-0019", "tui"]
+++

## 问题

Ghostty 1.3.1 上 inline viewport 仍出现输入区/背景覆盖或错位；tmux 抓屏正常不能代表 Ghostty。

## 验收

- [ ] 在真实 Ghostty 中抓屏：提交前后、picker 展开/关闭、滚动多轮均不覆盖 transcript。
- [ ] 固定/动态 viewport 的 cursor/clear 语义有终端级回归测试。
- [ ] 通过后移除 Ghostty 默认 linear fallback。
- [ ] 保持 `NIUCODE_TUI_LINEAR=1` 作为显式 fallback。

## 备注

当前 Ghostty 默认 linear；`NIUCODE_TUI_INLINE=1` 可强制 inline 复现。
2026-10-21：linear 的 startup 重叠已单独修复——启动信息统一走 stdout、显式 flush、workspace 只打印 project key；不再出现 prompt 覆盖 workspace 行。
