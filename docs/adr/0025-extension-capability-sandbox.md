+++
id = "adr-0025"
title = "ADR-0025：extension capability sandbox 与 worker pool"
kind = "adr"
status = "accepted"
order = 52
summary = "sidecar 默认跑在 bwrap 内：无网络、workspace 只读、fs_write/network 必须显式 capability；每插件 1-8 个 worker，mount 记录 capabilities/digest。"
+++

## 状态

Accepted · 2026-10-20

## 决策

1. manifest 增加 `capabilities = ["fs_read"|"fs_write"|"network"|"subprocess"]`，未知 capability 拒绝挂载。
2. 默认 `network off`、workspace read-only；`fs_write` 才 bind rw，`network` 才取消 `--unshare-net`。
3. sidecar 一律经 bwrap：只 bind 系统目录与 workspace，`/tmp` tmpfs，`/dev`/`/proc`，env clear + minimal setenv。
4. `workers` 1-8；每个 worker 独立进程、独立初始化；host round-robin 调度，单 worker 串行。
5. MountInfo/event 记录 capabilities、digest、workers；主 session 打印审计信息。
6. subagent 共享同一个 ExtensionHost，不按 subagent 复制进程。
7. `subprocess` 目前是审计声明，bwrap PID namespace 不等价于禁止子进程；后续再收紧。

## 后果

- 扩展不再默认获得网络/写权限；缺失 bwrap 时挂载失败，不静默降级。
- worker pool 提升并发，但每个 worker 有进程/内存成本。
- bwrap 只 bind 系统目录，Python 等解释器可用；依赖特殊路径的插件需要显式扩展 bind 策略（后续）。
