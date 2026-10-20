+++
id = "adr-0022"
title = "ADR-0022：Ghostty 等终端先用 linear fallback"
kind = "adr"
status = "accepted"
order = 49
summary = "inline ratatui 在 Ghostty 上仍有覆盖/坐标问题；Ghostty 默认回退 linear，inline 仅在显式 opt-in 或其他终端使用，直到有终端级回归测试。"
+++

## 状态

Accepted · 2026-10-20

## 背景

inline viewport 依赖 cursor-position 查询和终端 clear 语义。tmux 抓屏验证后，Ghostty 1.3.1 仍出现输入区覆盖/错位；继续在默认路径上试布局会反复打断用户。

## 决策

1. Ghostty（`TERM_PROGRAM=ghostty`、`TERM=*ghostty*` 或 `GHOSTTY_RESOURCES_DIR`）默认使用稳定 linear editor。
2. 其他终端继续 inline；`NIUCODE_TUI_LINEAR=1` 强制 linear。
3. `NIUCODE_TUI_INLINE=1` 可强制 inline，用于复现/验证，不是默认承诺。
4. 在 M5-006 完成 Ghostty 屏幕级回归测试前，不把 inline 重新设为 Ghostty 默认。
5. reasoning 可见性不受 fallback 影响：两种 editor 都显示 `◇ thinking` + dim 流式文本。

## 后果

- Ghostty 用户先得到无覆盖的稳定交互；代价是暂时没有 inline 紧凑布局。
- inline 代码保留但状态为 review，不作为完成承诺。
- 需要真实 Ghostty/tmux 抓屏测试，而不是只用 ANSI 解析器。
