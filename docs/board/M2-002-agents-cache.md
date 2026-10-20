+++
id = "M2-002"
title = "AGENTS.md 分层发现与缓存指标"
status = "backlog"
milestone = "M2"
module = "context"
size = "M"
priority = "P2"
updated = "2026-10-20"
depends_on = ["M2-001"]
+++

## 范围

- 从 cwd 向上发现 `AGENTS.md`，按距离合并，近者优先。
- system prompt 只保留稳定前缀；易变信息放末尾。
- 记录 cached / input tokens，按 turn 汇总命中率。
- 看板 Runs 视图读取 metrics JSONL。

## 验收

- 子目录规范能覆盖根目录规则。
- 修改 `AGENTS.md` 后下一 turn 生效，无需重启。
- 缓存命中率可在 session 事件里看到。
- 前缀变化导致缓存失效时能定位原因。

## 记录

缓存命中率是 context 工程质量最直接的指标。没有指标就不要谈优化。
