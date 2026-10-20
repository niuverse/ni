+++
id = "M5-004"
title = "Isolated /init proposal and apply"
status = "done"
milestone = "M5"
module = "cli"
size = "M"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M5-003"]
docs = ["adr-0020", "tui", "architecture"]
+++

## 范围

- 独立只读 init session；8 turns / 120s。
- 工具白名单 read/grep/glob；无 subagent/extension。
- 固定章节 AGENTS.md；确定性 scan + 模型生成。
- proposal 事件；`/init apply` 显式写入。
- 备份到全局 project backups；plan mode 只生成。

## 验收

- [x] `init_proposal { content, path }` 事件；resume 恢复。
- [x] `/init` 预览（新文件全文 / 已有文件 unified diff）。
- [x] `/init apply` 走 Toolbox、read-before-write、policy。
- [x] 覆盖前备份 `backs/AGENTS-<ts>.md`。
- [x] plan mode 生成成功、apply 被拒绝。
- [x] 模型输出缺章节时回退到 scan 模板。
- [x] 软目标 100 行 / 6 KiB 警告。
- [x] `/init apply` 后再 reload instructions。

## 记录

- PTY + replay fixture：proposal/apply 成功；已有 AGENTS.md 的备份生成；plan mode apply 被拒绝。
- 单测覆盖 fallback 固定章节、fence 清理、required sections。
- 未做：模型生成内容的语义 diff 审查；init proposal 的 TUI 选择器/编辑器内联修订。
