+++
id = "adr-0007"
title = "ADR-0007：Diff 预览归工具层，渲染归客户端"
kind = "adr"
status = "accepted"
order = 26
summary = "审批 diff 由工具层按真实文件生成结构化预览；loop 只转发，TUI 用宽度感知 pager 渲染。"
+++

## 状态

Accepted · 2026-10-20

## 背景

M1-001 的审批预览只是工具输入片段：`write` 把 `content` 前 40 行加上 `+`，`edit` 把 `old` / `new` 各显示一行。它看不到文件原来的内容，不是 unified diff，也没有行号、hunk 和上下文。M1-003 要求在审批前看到本次写入的完整 diff，并且大 diff 不刷屏、能按文件跳转、终端宽度变化后可以重排。

问题在于谁来算 diff：

- loop 知道 tool call，但不知道工作区路径策略，也不应该读文件。
- TUI 知道终端宽度，但不应该重新实现 path resolve 和 read-before-write，否则会出现第二套安全策略。
- 工具层已经拥有路径解析、沙箱判断和 `write` / `edit` 的真实语义。

## 决策

1. `Toolbox::preview(name, input) -> ChangePreview` 在工具层生成**无副作用**的结构化预览：文件、hunk、行号、增删行和 warning。它读取真实文件内容，但不修改 observed 集合或磁盘。
2. `ApprovalRequest.preview: Option<ChangePreview>` 携带预览；loop 不解析、不渲染、不拼接 diff 字符串。
3. 渲染归客户端。TUI 的 `diff` 模块把预览布局成宽度感知的行；`review` 模块只在审批时进入 alternate-screen pager，支持滚动、按文件跳转和 resize 重排。headless 用同一布局输出纯文本。
4. pager 是审批 overlay，不把 TUI 改成常驻全屏应用；`a` / `d` / `s` 决策后回到线性会话。
5. 工具输出折叠留在 UI。`AgentEvent::ToolResult` 带完整 output，TUI 超过 12 行或 2 KB 只显示摘要和 `:last` 提示；session 日志仍保留完整 output。

## 后果

### 正向

- diff 与真实文件一致，审批看到的是将要发生的修改。
- 行号、标记和宽度处理只实现一次，headless 与 TUI 共享同一套布局。
- 工具层继续拥有路径和沙箱语义；UI 不复制安全逻辑。
- 大 diff 可导航，窄终端可以退化为纯文本预览。

### 负向

- 预览与执行之间存在 TOCTOU：execution 会重新读取并校验，预览只代表审批时刻。
- 预览读取大文件会占用内存；当前接受，后续可以加预算或分页。
- pager 增加终端状态管理；用 guard 保证异常路径恢复。
- `AgentEvent` 携带完整 output，事件观察者需要自己决定折叠。

## 备选方案

- **loop 生成 diff 字符串**：简单，但 loop 要读文件、处理路径，且无法按终端宽度渲染。
- **UI 自己读文件**：能拿到宽度，但复制 path policy、read-before-write 和沙箱判断，容易与执行不一致。
- **引入 ratatui 全屏 TUI**：长期可能，但 M1 的交互面还没稳定；先做审批 overlay，避免为 pager 重写整个会话界面。
- **调用外部 diff / pager**：引入子进程和终端接管，离线和跨平台更复杂，也无法和审批按键整合。

## Revisit 条件

1. 需要一次审批多个文件，或出现 rename / delete / patch 工具，预览模型要扩展。
2. diff 规模超过内存预算，需要分页读取或外部 diff。
3. 线性 TUI 的滚动和历史检索成为瓶颈，考虑 ratatui 全屏。
4. headless `--json` 需要把 preview 作为稳定协议字段。
