+++
id = "adr-0014"
title = "ADR-0014：Profile、Plan mode 与 Goal 的目标分离"
kind = "adr"
status = "accepted"
order = 41
summary = "用户只看 profile（ask/review/full-access/ci）；plan 是运行时 mode；goal 是会话目标标签；access/approval/reviewer/risk 留在实现层。"
+++

## 状态

Accepted · 2026-10-20

## 背景

把 access、approval、reviewer、risk 四个轴直接暴露给用户会产生 3×4 种组合，难以理解；而 Codex/Claude Code/Gemini CLI 都用 mode/profile 包装底层策略。plan 是工作流状态，不是权限档位；goal 是目标，不是权限。

## 决策

1. **用户只选 profile**：`:ask`（workspace + 人工审批，默认）、`:review`（workspace + auto-review）、`:full-access`（无沙箱，显式开启）、`:ci`（workspace + never）。**没有 `:plan`。**
2. **profile 是 preset**：底层仍是 access + approval + reviewer + risk；高级用户可以 `--access` / `--approval` / `--reviewer` 覆盖，TUI `/permissions` 显示 effective policy。
3. **`/plan` 是运行时 mode**：进入后 `effective_access = min(base_access, read-only)`，写/命令/联网被阻止或询问；生成 plan artifact；`/plan off` 或 `/plan approve` 退出并恢复 base profile。plan 只收紧，不放宽。
4. **`/goal` 是目标标签**：一个 session 一个 active goal；写入 `goal_set` / `goal_cleared` 事件；进入 context；resume 恢复；不改变权限。`/goal run` evaluator loop 是 backlog。
5. **subagent capability profiles**：Explore = read-only + 只读插件 allowlist；Worker = workspace + write/edit/bash + 只读插件 allowlist；两者都不默认 full-access。C（full inheritance）是 backlog。
6. **W1 写入隔离**：Worker 与主 session 共享 workspace；全局写锁串行；write/edit 有 path scope；bash 串行。worktree 是 backlog。
7. **risk 是工具元数据**：`low/medium/high/unknown`，用于 auto-review 和 subagent 授权；用户不直接配置。高风险默认只在主 session/人工确认；unknown 按高风险处理。
8. **auto-review 是 profile 行为**：`:review` 使用独立 reviewer，无工具权限，只能 allow/deny/escalate；不能改 access mode；高风险/unknown 升级给人。

## 后果

### 正向

- 用户只需要理解 4 个 profile 和 2 个命令（`/plan`、`/goal`）。
- plan 不会和权限 profile 混在一起；goal 不会改变权限。
- 高级用户仍有完整表达力；审计事件记录 effective policy。
- subagent 能查只读插件、能写代码，但默认不接触 full-access/destructive 外部操作。

### 负向

- profile 隐藏了底层组合，出错时需要 `/permissions` 解释。
- `/plan` 是模式，不是安全边界本身；必须配合 allowed_tools/sandbox 才可靠。
- auto-review 有 reviewer 被 prompt injection 影响的风险；需要独立 context、无工具、日志和升级规则。
- C/`/goal run`/worktree 都留了 backlog，短期能力有限。

## 备选方案

- **两轴直接暴露**：表达力强，但用户认知成本高。
- **只用一个 risk 档位**：无法表达 read-only/workspace/full-access 的能力差异。
- **`:plan` 作为 profile**：和 `/plan` mode 重复，且 plan 结束要恢复原 profile，不适合做静态档位。
- **`/goal` 立即自动循环**：容易意外烧 token；先做标签，loop 单独开关。

## Revisit 条件

1. `/goal run` 需要 evaluator 模型、预算、暂停/恢复和 no-progress 检测。
2. Worker 并行写需求增加，引入 worktree 或 overlay。
3. Profile 数量继续增长，需要用户自定义 profile 文件。
4. auto-review 的误判/注入风险需要更强隔离或 human-only 高风险策略。
