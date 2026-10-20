+++
id = "adr-0006"
title = "ADR-0006：Linux 沙箱首选 bwrap，landlock/seccomp 后置"
kind = "adr"
status = "accepted"
order = 25
summary = "三轴策略独立；Linux 用 bubblewrap 提供文件系统与网络隔离，缺少 bwrap 时降级到逐工具审批。"
+++

## 状态

Accepted · 2026-10-20

## 背景

`ni exec` 已经有 bash 进程组、超时和取消，但 bash 仍能写工作区外、访问网络。M1-002 需要把“能写哪、能连哪、什么时候问人”变成可执行策略，而不是文档承诺。

候选：

- 手写 landlock + seccomp：零外部依赖，但需要自己处理 ABI、规则和 pre-exec/fork 安全；第一版风险高。
- bubblewrap（bwrap）：成熟的 Linux namespace 沙箱，支持只读根、工作区 bind、网络 namespace；本机已安装并验证可用。
- 纯 approval：没有隔离，只能靠人。

## 决策

把三轴拆开：

```text
sandbox:  read-only | workspace-write | full-access
network:  on | off
approval: ask | on-failure | never
```

Linux 第一后端用 bwrap：

- `read-only`：`--ro-bind / /`，不 bind 工作区；文件工具也拒绝 write/edit。
- `workspace-write`：只读根 + `--bind <cwd> <cwd>`。
- `full-access`：不套 bwrap（`network=on`），或只套网络 namespace（`network=off`）。
- `network=off`：`--unshare-all`；`network=on` 加 `--share-net`。
- bash 仍走独立进程组；超时/取消杀整组。

如果 bwrap 不可用：

- `approval=never`：启动时直接失败。
- `approval=on-failure`：提升为 `ask`，每个变更型工具前询问；显式提示网络策略没有被执行。
- `approval=ask`：同样逐工具询问。
- `full-access + network=on`：不需要 bwrap，不存在降级。

每次运行把有效策略写成 `policy` 事件；人工审批写成 `approval` 事件。拒绝执行的工具结果也落盘。

## 后果

### 正向

- 默认 `workspace-write + network=off`，不依赖人肉自觉。
- bwrap 是成熟组件，避免第一版手写 seccomp 规则。
- 三轴是独立枚举，未来换 landlock/seccomp 后端不影响 CLI 和事件。
- 没有 bwrap 的平台有明确降级路径，不会假装安全。

### 负向

- Linux 运行需要安装 `bubblewrap`；最小容器可能没有。
- bwrap 仍是同一内核上的 namespace 隔离，不是 VM；可读系统文件，内核攻击面仍在。
- 还没有 seccomp syscall 过滤；`landlock` 作为无 bwrap 依赖的后端后续补。
- macOS seatbelt 未实现；目前会走审批降级。
- `workspace-write` 只允许写 cwd，cargo registry / `~/.cargo` 等缓存写入会失败；需要后续加可配置 writable roots。

## 备选方案

- **landlock + seccomp 先行**：更轻，但实现和测试成本高；作为第二后端。
- **只做 approval**：不满足“workspace-write 无法写外部”的验收。
- **容器/VM**：隔离更强，但启动和分发成本不适合当前单二进制目标。

## Revisit 条件

1. 需要无 bwrap 的静态二进制分发。
2. 需要 syscall 级过滤或更细的 writable roots。
3. macOS 成为一等平台，需要 seatbelt profile。
