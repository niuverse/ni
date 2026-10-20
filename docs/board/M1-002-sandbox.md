+++
id = "M1-002"
title = "Sandbox 三轴与原生隔离"
status = "ready"
milestone = "M1"
module = "sandbox"
size = "L"
priority = "P1"
updated = "2026-10-20"
depends_on = ["M0-007"]
+++

## 范围

- `sandbox`：read-only / workspace-write / full-access。
- `approval`：ask / on-failure / never。
- `network`：on / off。
- Linux 用 landlock + seccomp，macOS 用 seatbelt；先做一个平台。
- 路径策略与 shell 隔离分开实现，不假装进程内检查是安全边界。

## 验收

- workspace-write 下无法写工作区外路径。
- 网络关闭时子进程连接被拒绝。
- 逃逸尝试留下审计事件，而不是静默失败。
- 无沙箱平台明确降级到强制审批，并提示原因。

## 记录

安全模型要能一句话解释清楚：能写哪、能连哪、什么时候问人。
