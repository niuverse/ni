+++
id = "adr-0019"
title = "ADR-0019：inline ratatui 输入层，保留原生 transcript"
kind = "adr"
status = "accepted"
order = 46
summary = "ratatui 只接管底部输入/picker/footer；不进入 alternate screen；鼠标捕获只在 picker；transcript 仍走终端 scrollback。"
+++

## 状态

Accepted · 2026-10-20

## 背景

线性 TUI 的排版由手写 crossterm 拼装，输入、菜单、状态分散在 3k 行里。全屏 ratatui 能解决排版，但会失去终端原生 scrollback、鼠标选择/复制；用户明确要求自然复制。

## 决策

1. 引入 ratatui，但只使用 **inline viewport**，不切换 alternate screen。
2. transcript 仍是普通终端输出；ratatui 只负责底部 picker/help、input、footer。
3. 鼠标捕获只在 picker 打开时开启；普通输入和 transcript 保留原生选择/复制。
4. 不引入 tui-textarea/markdown 等第二套 UI 框架；输入/候选自己维护。
5. 迁移期间保留 `NIUCODE_TUI_INLINE=1` opt-in 和 linear fallback；parity 完成后再切默认，不长期双轨。
6. 全屏 ratatui + copy mode 作为未来可选，不作为默认。

## 后果

- 排版/状态栏可以独立演进，transcript 的复制/scrollback 不回归。
- inline viewport 的高度、光标、picker 命中测试需要自己处理。
- 迁移期存在两套输入实现，完成后删除 linear 默认路径。
