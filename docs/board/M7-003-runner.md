+++
id = "M7-003"
title = "Isolated runner, metered gateway and four harness adapters"
status = "done"
milestone = "M7"
module = "benchmark"
size = "L"
priority = "P1"
updated = "2026-10-21"
completed = "2026-10-21"
depends_on = ["M7-002"]
docs = ["benchmark-plan", "adr-0029"]
+++

## 范围与验收

四方两轨在冻结镜像的 dev/smoke 全部通过；190 个 workspace 测试、五项 Python runner/audit 测试、fmt 与 clippy 通过。网关预算、取消、原始测试恢复和镜像来源校验有证据。

- [x] 范围已实现，并有独立验收证据。
- [x] 完整仓库检查与 dashboard 同步。
