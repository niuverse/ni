+++
id = "adr-0001"
title = "ADR-0001：Rust 作为核心语言"
kind = "adr"
status = "accepted"
order = 20
summary = "性能优先不等于 loop 快几毫秒；Rust 换来的是启动、内存、分发和原生沙箱。"
+++

## 状态

Accepted · 2026-10-20

## 背景

niucode 是一个单二进制 CLI：长会话、流式子进程、沙箱、可能的多子代理。语言选型要在两端做取舍：

- **运行时**：harness 自身耗时（SSE 解析、JSON、日志、调度）通常只占一个 turn 的 1–5ms，而模型 TTFT + 生成是 1–3s。换语言不会让 agent 明显变快。
- **工程形态**：冷启动、常驻内存、单二进制分发、进程组/信号控制、原生沙箱，这些是 Rust 的真实优势。
- **开发迭代**：Rust 的编译反馈和 async 设计摩擦，会让早期功能吞吐慢 1.5–2x；这是"写功能慢"，不是"运行时慢"。

## 决策

核心使用 Rust，workspace 拆分小 crate：

```text
crates/
  niucode-core/       # 事件、会话、loop、context
  niucode-cli/        # 参数与生命周期
  niucode-provider/   # 后续：wire adapters
  niucode-tools/      # 后续：工具
  niucode-sandbox/    # 后续：策略与平台隔离
  xtask/         # 仓库工具（看板构建）
```

## 后果

### 正向

- 单静态二进制，启动 2–10ms，无 runtime 依赖。
- 常驻内存低，多 session / 子代理时线性成本可控。
- 进程组、超时、信号、fd 管理语义明确，取消链路可靠。
- landlock / seccomp / seatbelt 可直接调用，不需要 native addon 或外部包装进程。
- 事件、状态机、工具 schema 适合强类型，后期重构成本低。

### 负向

- 编译反馈比 TS 慢一个数量级。
- async 流式状态机在接口未定型时摩擦大。
- OpenAI / Anthropic 的 Rust SDK 不如 TS/Python 成熟，SSE 和事件映射要自己写。
- TUI 迭代比 Ink/React 慢。

## 缓解

1. workspace 拆小 crate，`niucode-core` 不依赖 CLI / TUI，增量编译范围小。
2. `bacon` / `cargo-watch` + `cargo check` 作为内循环；链接器用 mold/lld。
3. 控制 proc-macro 依赖，provider 边界允许 `serde_json::Value`，内部强类型。
4. headless first，TUI last；loop / tools 用 scripted provider 离线测试。
5. context 逻辑写成纯函数 + fixture，改 prompt 策略不需要重编译整个 workspace。

## 备选方案

- **TypeScript / Bun**：迭代最快，官方 SDK 最全，TUI 生态好。运行时足够，但单二进制、内存和原生沙箱弱。若接下来数周的主要目标是高频试 context 策略，这是更快的选择。
- **Rust core + TS TUI（JSON-RPC）**：长期可行，但 v0 多一套协议、进程生命周期和错误映射，违背 minimal。等事件协议稳定后再考虑拆分。
- **Python**：原型最快，但分发、并发、类型约束和长会话内存都不占优。

## 备注

"Rust 慢一倍"指的是开发迭代速度，不是运行时。性能优先应当优化 prompt cache、工具输出体积、并行工具和 compaction，而不是期待换语言让模型响应变快。
