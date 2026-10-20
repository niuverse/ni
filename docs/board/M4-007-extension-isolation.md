+++
id = "M4-007"
title = "Extension capability sandbox + worker pool"
status = "done"
milestone = "M4"
module = "extensions"
size = "L"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M4-002"]
docs = ["adr-0025", "adr-0013"]
+++

## 范围

- manifest capabilities（fs/network/subprocess）与 worker pool。
- bwrap 默认无网络、只读 workspace；高风险能力显式授权。
- subagent 共享 session host；mount 审计 capabilities/digest。

## 验收

- [x] 默认 network off；越权网络调用失败（sidecar 测试）。
- [x] 默认 workspace 只读；`fs_write` 才允许写（sidecar 测试）。
- [x] `workers=2` 实际启动两个 worker，并发调用落到不同 worker。
- [x] `extension_mount` 记录 capabilities/digest/workers。
- [x] subagent 共享 host，不复制插件进程（既有 host 路径）。
- [x] 未知 capability 拒绝挂载。

## 记录

- bwrap bind `/usr` `/bin` `/lib` `/lib64` `/etc`、workspace、tmpfs `/tmp`；env clean。
- worker index 通过 `NIUCODE_EXTENSION_WORKER` 注入，测试用 PID namespace 内 PID 不可区分。
- 未完成：manifest 签名/远程分发、`subprocess` 强隔离、动态 bind 白名单。
