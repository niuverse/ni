+++
id = "M5-007"
title = "Codex-compatible skills discovery and trust"
status = "review"
milestone = "M5"
module = "cli"
size = "M"
priority = "P1"
updated = "2026-10-20"
depends_on = ["M5-002", "M4-007"]
docs = ["adr-0024", "tui"]
+++

## 范围

- `~/.agents/skills` + 项目 `.agents/skills` 的 `SKILL.md` 发现。
- 最小 YAML frontmatter parser。
- 全局默认可信；项目 digest 信任。
- `/skills` 列表与 trust/untrust。
- `$name` 激活块 + 只读 roots，正文按需 read。

## 验收

- [x] 发现/解析/排序；无项目 `.niucode/skills`。
- [x] 项目 skill 未信任时不激活。
- [x] `/skills trust|untrust` 写 digest；全局无需信任。
- [x] `$name` 只注入路径/描述；read roots 允许读 SKILL.md。
- [ ] 项目信任必须经过 M4-007 capability gate；当前为 review。
- [ ] `$` 输入补全（下一轮）。

## 记录

- `skills.rs` 负责 discovery/YAML/digest/trust store。
- Toolbox `read_roots` 只放宽 read；write 仍 workspace 严格。
- 单测覆盖项目未信任/信任后激活与 read roots。
