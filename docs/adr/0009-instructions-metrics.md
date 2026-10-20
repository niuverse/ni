+++
id = "adr-0009"
title = "ADR-0009：分层 AGENTS.md 与缓存指标留在事件日志"
kind = "adr"
status = "accepted"
order = 28
summary = "AGENTS.md 每次 run 按 cwd 向上发现并追加到稳定前缀；缓存命中率从 turn_end.usage 派生，不引入 Runs 看板。"
+++

## 状态

Accepted · 2026-10-20

## 背景

M2-002 要解决两件事：子目录规范要能覆盖根目录规则，缓存命中率要能解释。原始卡片还提到“看板 Runs 视图读取 metrics JSONL”，但这与两条既有约束冲突：

- 项目看板的导航固定为 Architecture / Work / Docs / Decisions，第一屏是架构，不是运行时监控。
- 事件日志是唯一持久真相；再写一份 metrics JSONL 会制造第二份可漂移的状态。

同时，prompt cache 的前缀稳定性要求 system prompt 的结构可解释：稳定部分在前，容易变化的指令在后。

## 决策

1. **每次 run 从 cwd 向上发现 `AGENTS.md`。** 顺序是 cwd、父目录……根目录；近者先列，并明确说明近者优先。每个文件上限 32 KiB，总上限 64 KiB；超出时截断并 warning。
2. **指令追加在稳定 system prompt 之后。** base prompt 保持稳定前缀，`AGENTS.md` 内容放在末尾。每次 `run_with_history` 重新加载，所以修改文件后下一次用户 turn 生效，无需重启。
3. **记录 `instructions` 事件。** 事件包含加载的文件路径和合并文本的 FNV-1a digest；只有实际加载到文件时才写入。缓存命中率下降时，可以对照 digest 判断是否是指令变化导致前缀失效。
4. **缓存指标从 `turn_end.usage` 派生。** `Usage::cache_hit_percent()` 用 `cached_tokens / input_tokens` 计算；TUI、exec 和 `session show` 直接显示。不写独立 metrics JSONL。
5. **Runs 视图不进入离线项目看板。** 运行时的 runs / metrics 浏览属于未来 runtime viewer，和架构阅读面分离；看板继续只读、离线、四视图。
6. **不为每次 assistant turn 重载指令。** 指令在一次 run 内保持稳定，避免同一次任务中 prompt 前缀反复变化。

## 后果

### 正向

- 子目录规则按距离覆盖根目录规则，顺序可测试。
- `AGENTS.md` 修改在下一 turn 生效，不需要重启进程。
- instructions digest 让“为什么 cache 命中率掉了”有可查的证据。
- 缓存命中率来自真实 usage 事件，不需要第二份指标文件。
- 看板保持 architecture-first，不引入运行时可变的进度/监控面。

### 负向

- `AGENTS.md` 一旦变化，system prompt 后缀变化，prompt cache 大概率失效；这是正确性优先的代价。
- 一次 run 内不会感知指令变化；超长 run 需要用户重新提交 turn。
- 还没有历史 Runs 视图，跨 session 的缓存趋势需要手工读 JSONL。
- FNV-1a digest 只用于定位变化，不是密码学完整性校验。

## 备选方案

- **写 metrics JSONL**：查询方便，但会引入第二份持久状态，且与 `turn_end.usage` 重复。
- **在项目看板加 Runs 视图**：与 architecture-first 和固定导航冲突；真要做应放进 runtime viewer 并单独写 ADR。
- **只读根目录 `AGENTS.md`**：实现简单，但不满足子目录规范覆盖根目录规则。
- **每个 assistant turn 重载指令**：能更快感知修改，但会让 prompt 前缀在任务中途变化，破坏缓存。
- **把指令放进 user message**：会混淆用户输入和仓库规范，也不利于稳定前缀。

## Revisit 条件

1. 出现 runtime viewer，需要跨 session 的 Runs / metrics 视图。
2. 需要更精细的缓存诊断（按 system / tools / history 分段统计）。
3. 指令来源扩展到 include、工作区外配置或多仓库继承。
4. digest 需要密码学校验或跨机器比较，FNV-1a 不再够用。
