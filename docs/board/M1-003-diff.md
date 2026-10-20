+++
id = "M1-003"
title = "Diff 预览与输出折叠"
status = "done"
milestone = "M1"
module = "tui"
size = "M"
priority = "P2"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M1-001"]
docs = ["adr-0007", "tui"]
+++

## 范围

- 统一 diff 渲染：文件头、hunk、增删行、上下文折叠。
- 工具输出超过阈值时折叠，只显示摘要和"展开"提示。
- 审批前展示本次写入的完整 diff。

## 验收

- [x] 大 diff 不刷屏，能按文件跳转。
- [x] 行号与新增/删除标记对齐。
- [x] 终端宽度变化后重排不破版。

## 记录

工具层（`niucode-tools/src/preview.rs`）用真实文件内容生成结构化 `ChangePreview`；loop 只转发，不再拼接预览字符串。

客户端分层：

- `niucode-cli/src/diff.rs` 负责宽度感知布局：文件头统计、`@@` hunk、old/new 双行号、`+/-` 标记、上下文半径 3、tab 展开、CJK 宽度和终端转义清洗。
- `niucode-cli/src/review.rs` 在 `--approval ask` 时提供 alternate-screen pager：`j/k` 滚动、PageUp/PageDown、`g/G`、`n/p` 按文件跳转、resize 重排，`a/d/s` 决策。窄于 40 列或低于 6 行退回纯文本。
- 当前 `write` / `edit` 每次只改一个文件；pager 的多文件跳转已由布局和导航测试覆盖，等 patch 类工具接入后直接复用。
- `niucode-cli/src/tui.rs` 对超过 12 行或 2 KB 的工具输出折叠为摘要 + `:last to expand`；`:last` 重印最近一次完整输出。session 日志仍保留完整 output。

离线覆盖：preview 结构、行号对齐、宽度边界、CJK、ANSI 清洗、pager 文件跳转、输出折叠。另用 PTY 跑过 allow / deny / resize / `:last` 四条真实交互路径。

决策见 ADR-0007。预览与执行存在 TOCTOU：预览只代表审批时刻，执行会重新读取并校验。
