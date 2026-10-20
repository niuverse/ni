+++
id = "M0-006"
title = "文件工具：read / write / edit / grep / glob"
status = "done"
milestone = "M0"
module = "tools"
size = "L"
priority = "P0"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M0-001"]
docs = ["architecture", "exec"]
+++

## 范围

- 五个文件工具 + 一个注册表；JSON Schema 描述输入。
- `read` 返回行号，单次上限 2000 行 / 50KB，截断时给出续读方式。
- `edit` 用精确字符串替换，要求唯一匹配，返回小 diff。
- `grep` / `glob` 结果有上限，并报告总数。
- read-before-write 策略：未读过的文件拒绝写。

## 验收

- [x] 非唯一 `old_string` 被拒绝，并提示增加上下文。
- [x] 未读文件 `write` / `edit` 返回可操作的错误。
- [x] 超大文件读取被截断，模型能按 offset 继续。
- [x] `grep` / `glob` 有结果上限，尊重 `.gitignore`，跳过 `target` / `.git`。
- [x] `edit` 返回 unified diff 预览。

## 记录

五个文件工具已落地；`grep`/`glob` 用 `ignore` + `regex`/`globset`，不依赖外部 ripgrep。工具描述仍是 prompt 表面。
