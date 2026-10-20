+++
id = "adr-0024"
title = "ADR-0024：Codex 兼容 SKILL.md 与 digest 信任"
kind = "adr"
status = "accepted"
order = 51
summary = "skills 是声明式 prompt 扩展；~/.agents/skills 默认可信，项目 .agents/skills 需 canonical cwd+digest 绑定；$name 只注入激活块/路径，正文按需 read。"
+++

## 状态

Accepted · 2026-10-20

## 决策

1. 路径只认 `~/.agents/skills` 与项目 `.agents/skills`；不创建 `.niucode/skills`。
2. 每个 skill 是目录下的 `SKILL.md`（也接受单个 `.md`），frontmatter 只维护 `name`/`description`。
3. 全局 skills 默认可信；项目 skills 默认禁用，`/skills trust <name>` 显式写入 digest 到全局 project 的 `skills-trust.json`。
4. `$name` 激活时只把名称/描述/绝对路径加入 system prompt；正文由 agent 用 `read` 按需读取。
5. 为支持全局 skill，Toolbox 增加只读 roots；write/edit 仍严格限制在 workspace。
6. 不自动执行 skill 内的脚本；skills 不是代码插件。
7. 项目 skill 信任写入在 M4-007 安全 gate 完成前保持 review。
