+++
id = "adr-0020"
title = "ADR-0020：/init 隔离生成 + 显式 apply"
kind = "adr"
status = "accepted"
order = 47
summary = "init 在独立只读 session 中扫描/生成；完整 proposal 写入事件；/init apply 才经 Toolbox 写 AGENTS.md，并备份到全局 project。"
+++

## 状态

Accepted · 2026-10-20

## 背景

静态模板无法生成项目特定的命令、架构和边界。让主 session 直接写 AGENTS.md 又会让生成过程污染对话、跳过审批/策略，并可能覆盖人工内容。

## 决策

1. `/init` 在独立 session 中跑模型：
   - tools 仅 `read/grep/glob`；
   - sandbox read-only，network off；
   - 8 turns，120s timeout；
   - 无 subagent、无 extension；
   - 固定章节：Purpose / Commands / Architecture / Workflow / Guardrails / Current focus。
2. 最终 assistant 消息就是完整文件；不做额外写工具。
3. 完整 proposal 写入父 session 的 `init_proposal { content, path }` 事件；resume 可恢复。
4. 首次只预览，必须 `/init apply` 才写；apply 走 Toolbox 的 read-before-write/sandbox policy。
5. 覆盖已有 AGENTS.md 前备份到 `~/.niucode/projects/<key>/backups/AGENTS-<ts>.md`。
6. plan mode 只生成，不 apply；read-only sandbox 同样只能生成。
7. 超过 100 行 / 6 KiB 只警告，不截断。

## 后果

- generation 与 mutation 分离；proposal 可 review、重试、跨 resume。
- 模型不可用/输出缺章节时回退到确定性 scan 模板。
- proposal 只保存在事件日志；项目根不出现 `.proposed`。
- 现有 AGENTS.md 通过 prompt + fallback 合并，人工内容优先。
