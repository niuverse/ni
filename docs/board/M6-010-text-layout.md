+++
id = "M6-010"
title = "Unify terminal text layout and remove display-only editors"
status = "done"
milestone = "M6"
module = "tui"
size = "M"
priority = "P1"
updated = "2026-10-21"
completed = "2026-10-21"
depends_on = ["M6-002", "M6-006"]
docs = ["tui", "terminal-acceptance", "adr-0028"]
+++

## 范围

用户选择架构审查中的统一文本布局，授权继续清理重复代码并补回归。
`text_layout` 统一字素换行、控制字符可视化、tab 列宽和裁剪；Composer 仅持有编辑状态。
详情、审批 diff、流式预览和 inline transcript 直接使用布局，删除三处临时 Composer。
linear transcript 保持终端原生软换行，复用同一控制字符/制表符规则。

## 验收

- [x] 静态输出不分配 undo/history/cursor map；只有输入布局生成光标位置映射。
- [x] 满行输出不额外插入空行；显式空行和结尾换行保留，输入仍预留行尾插入位置。
- [x] 中文、组合字符、ZWJ emoji、0/1 列宽、tab 和不可信控制字符有回归。
- [x] diff 的行号/增删标记保持；字素不在 gutter 换行时拆开。
- [x] 每个输出逻辑行最多采用 1 MiB 源文本，在字素边界裁剪，显示统一标记，后续行保留。
- [x] 详情通过真实日志读取和 resize 回归，日志字节不变。
- [x] workspace tests、fmt、clippy、dashboard check 通过；PTY 验证两种模式的审批/取消/详情恢复。

验证：200 个 Rust 测试通过；生产代码净减 20 行，新增/改写回归覆盖共享布局及真实消费者。

不调整 history/approval 视图状态结构；该项是另一个架构候选。
不修改评测归档或模型行为，不以自动化检查关闭 M5/M6 实机 gate。
