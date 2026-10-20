+++
id = "roadmap"
title = "Roadmap 与学习路径"
kind = "doc"
status = "living"
order = 15
summary = "从 M0 到可用 coding harness：组件地图、自研/用库边界，以及每个阶段应该真正学会的东西。"
+++

## 目标

不是“所有 agent 功能”，而是一个**可靠的单 agent coding harness**：

> 给一个真实 repo 和一个 issue，agent 能定位、修改、跑测试、给出 diff；失败能 resume；同一 fixture 可以离线回归。

平台层（MCP、插件、subagent、多 provider、长期记忆）不是“完整”的必要条件；先证明单 agent 闭环。

## 组件地图

- **Runtime / loop**（M0-008）— 自研 Command/Event 与 turn 状态机；用 tokio。学会：画出命令/事件生命周期，说明取消发生在哪。
- **Provider**（M0-005）— 自研流式归一化、SSE 映射、错误分类；用 reqwest。学会：解释重试边界和 token 账本。
- **Tools**（M0-006/007）— 自研注册表与六个工具；用 serde_json / ignore / globset。学会：证明工具输出不会撑爆上下文。
- **Context**（M2-001/002）— 自研 projection、compaction、AGENTS.md 合并。学会：解释“日志全量、上下文有损”。
- **State**（M0-002/009）— 自研 Event、JSONL、resume、fork；用 serde_json。学会：离线回放一次真实 turn。
- **Auth**（M0-011）— 自研 env → keyring → 0600 文件的解析与回退；用 OS keyring。学会：解释密钥为什么不能进日志，以及 keyring 失败时如何安全降级。
- **Safety**（M1-002）— 自研策略与审批链；用 landlock/seccomp 或 bubblewrap。学会：演示一次拒绝写入/联网。
- **Clients**（M0-004 / M1-001 / M1.5）— 自研 CLI 命令和事件到视图的映射；用 clap / ratatui。学会：同一事件流渲染两种界面。
- **Observability**（M2-002）— 自研 usage 事件、metrics JSONL。学会：定位一次慢 turn。
- **Evaluation**（M0-009）— 自研 scripted provider、fixture、golden transcript。学会：同一输入两次回放行为一致。

## 自研 / 用库的边界

判断标准：**“这部分决定 agent 的行为质量吗？”**

- 决定 → 自己写：loop、context、tool protocol、event log、approval、eval。
- 不决定 → 用库：HTTP、async runtime、CLI 解析、TUI 渲染、diff、glob、Markdown。
- 安全 → 策略自己写，OS 原语用库；不要手写 syscall。

## 顺序

```text
M0   CLI 骨架 / 事件日志 / auth / dashboard
     └─ M0-005 provider（scripted + OpenAI-compatible SSE）
     └─ M0-006 文件工具
     └─ M0-007 bash（超时、进程组、输出预算）
     └─ M0-008 loop + ni exec（SessionRuntime 边界）
     └─ M0-009 resume + fixture
M1   TUI / approval / sandbox / diff
M1.5 只读 Web Session Viewer（JSONL → turn/tool/diff）
M2   compaction / AGENTS.md / cache 指标 / Runs 视图
M3   replay / fork / subagent（视需要）
```

每个阶段都必须能离线测试，不能依赖真实 API 才能验证 loop。

## 学习检查点

完成一个阶段时，应该能回答：

- **Provider**：SSE 的 delta 如何合并成一条 assistant message？什么错误可以在 turn 边界重试，什么不能？
- **Tools**：工具输出如何截断？read-before-write 为什么必须是策略而不是提示词？两个写工具并发会发生什么？
- **Loop**：取消发生在 provider 流、工具执行、日志写入三个位置时，分别怎么处理？
- **Eval**：如何在不烧 token 的情况下复现一个 turn？回放和真实调用差在哪？
- **Sandbox**：路径校验、shell 隔离、网络关闭分别能防住什么，防不住什么？
- **Context**：压缩后模型看到的历史和日志里的历史为什么可以不同？cache 前缀为什么不能随手改？
- **Observability**：一个 turn 变慢时，如何区分模型、工具和 harness 自身？

## M0 完成定义

- `ni exec "<task>"` 能在真实 repo 里完成一个小修改并跑测试。
- session 可 resume；坏尾行不丢历史。
- 工具有输出上限；bash 能超时杀进程组。
- 至少一个 scripted provider 的离线回归测试。
- `cargo xtask dashboard --check` 通过，架构图和 ADR 反映当前实现。

## 明确后置

MCP、插件系统、多 provider 全家桶、长期记忆、调度器、多人协作、Web 编辑架构图。等出现真实需求，再写 ADR。
