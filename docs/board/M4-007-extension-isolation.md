+++
id = "M4-007"
title = "Backlog: extension capability sandbox + worker pool"
status = "backlog"
milestone = "M4"
module = "extensions"
size = "L"
priority = "P1"
updated = "2026-10-20"
depends_on = ["M4-002"]
docs = ["adr-0013"]
+++

## 范围

- manifest 声明 capabilities（fs/network/subprocess）与并发模型。
- bwrap/OS sandbox：默认无网络、只读 workspace；高风险能力需显式授权。
- 每插件 worker pool；read-only 调用可并发；worker 满时有界队列/背压。
- subagent 共享 session host（通过 ToolRegistryView），不复制插件进程。
- extension_mount 记录 capabilities/digest；resume 校验。

## 验收

- [ ] 默认 network off 可被测试验证；越权调用被拒绝。
- [ ] 两个 subagent 共享一个 host，插件进程数不随 subagent 增长。
- [ ] 并发调用受 worker pool 限制且可取消。
- [ ] 挂载前能力授权可审计。
