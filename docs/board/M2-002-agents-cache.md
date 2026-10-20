+++
id = "M2-002"
title = "AGENTS.md 分层发现与缓存指标"
status = "done"
milestone = "M2"
module = "context"
size = "M"
priority = "P2"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M2-001"]
docs = ["adr-0009", "exec", "tui"]
+++

## 范围

- 从 cwd 向上发现 `AGENTS.md`，按距离合并，近者优先。
- system prompt 只保留稳定前缀；易变信息放末尾。
- 记录 cached / input tokens，按 turn 汇总命中率。
- 缓存指标留在 session 事件；Runs 视图留给运行时 viewer（ADR-0009）。

## 验收

- [x] 子目录规范能覆盖根目录规则。
- [x] 修改 `AGENTS.md` 后下一 turn 生效，无需重启。
- [x] 缓存命中率可在 session 事件里看到。
- [x] 前缀变化导致缓存失效时能定位原因。

## 记录

实现位于 `crates/niucode-agent/src/instructions.rs`。

- 每次 `run_with_history` 从 cwd 向上发现 `AGENTS.md`：cwd、父目录……根目录，近者先列并明确说明近者优先。
- 单文件上限 32 KiB，总上限 64 KiB；截断和读取失败通过 `AgentEvent::Warning` 提示。
- 指令追加在稳定的 base system prompt 之后；一次 run 内不重载，修改文件后下一次用户 turn 生效。
- 加载到文件时追加 `instructions` 事件，记录 sources 和合并文本的 FNV-1a digest。缓存命中率下降时可以对照 digest 判断是否是指令变化导致前缀失效。
- `Usage::cache_hit_percent()` 从 `turn_end.usage` 的 cached / input 派生；TUI 每个 turn 和 `done` 行显示 `cache N%`，exec 同样显示，`session show` 显示每 turn 的 `in/out/cached/cache` 以及 instructions 文件数和 digest。
- 没有写独立 metrics JSONL。离线项目看板保持 Architecture / Work / Docs / Decisions 四视图，不加入运行时 Runs；见 ADR-0009。

测试覆盖：近者优先顺序、digest 变化、空文件忽略、修改 `AGENTS.md` 后第二个 turn 的 system prompt 更新且旧文本消失、instructions 事件落盘、缓存命中率计算。TUI / exec 的缓存显示也在 PTY 回归里检查。

决策见 ADR-0009。运行时 Runs / metrics 视图仍是后续工作，不在离线看板里硬塞。
