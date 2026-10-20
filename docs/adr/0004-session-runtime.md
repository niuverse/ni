+++
id = "adr-0004"
title = "ADR-0004：SessionRuntime 以 Command/Event 为边界"
kind = "adr"
status = "accepted"
order = 23
summary = "抽象会话而不是界面：CLI/TUI 进程内消费 Command/Event，Web 以后只是另一个 client。"
+++

## 状态

Accepted · 2026-10-20

## 背景

上层会出现 CLI、TUI 和 Web 三种交互界面。直觉做法是定义一个 `Frontend` trait，让 loop 调用 `render()` / `approve()`；但这样 loop 会知道“展示”，Web 需要桥接，`--json` 和多客户端也会变别扭。

三个界面真正共享的是**命令与事件的语义**，不是屏幕布局或 widget。

## 决策

定义 transport-neutral 的 `SessionRuntime`：

```text
Client ──► Runtime

  Submit { text }
  Approve { call_id, decision }
  Cancel { turn_id }
  Resume { session_id }
  Configure { model, budget, approval }

Runtime ──► Client

  RuntimeEvent =
    Durable(Event)          // 与 JSONL 日志一致
  | Delta { kind, text }    // 瞬时，不落盘
  | ApprovalRequest { ... } // 需要客户端决策
  | Status { ... }          // 运行状态
```

- CLI 和 TUI 现在**进程内**消费同一个 runtime。
- Web 第一版是只读 Session Viewer，直接读 JSONL；完整客户端以后通过 WebSocket/SSE 接同一个 contract。
- 不做 `Frontend` trait，不共享 widget 层。
- 不在 M0 做 daemon/JSON-RPC；等出现多客户端、远程访问或崩溃隔离需求再拆。

## 后果

### 正向

- loop 不依赖展示层；headless `--json`、TUI、Web 共用一套语义。
- approval / cancel / usage 是协议的一部分，不会漏在某个 UI 里。
- 未来拆 daemon 只需要给 runtime 包一层 transport，不重写 loop。

### 负向

- 需要定义事件的生命周期：哪些是 durable、哪些是瞬时、断线后如何恢复。
- 进程内 runtime 和未来 daemon 之间会有一段适配代码。
- Command 的幂等性和重放语义需要明确（尤其 `Approve` 与 `Cancel`）。

## 备选方案

- **`Frontend` trait**：简单，但把展示耦合进 loop，Web 和多客户端会变复杂。
- **现在做 daemon + JSON-RPC**：对 Web 最干净，但在 loop 尚未跑通时预付协议、认证、背压和进程生命周期成本。
- **每个 UI 各自调用 loop**：最快，但会出现三套取消、审批和错误处理，最终行为不一致。

## Revisit 条件

1. 需要同时连接多个客户端，或从远程/移动端控制同一个 session。
2. 需要把 agent 进程与 UI 进程隔离（崩溃恢复、资源限制）。
3. Command/Event 需要版本化并对外发布。
