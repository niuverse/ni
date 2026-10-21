+++
id = "M7-004"
title = "Public baseline runs and failure analysis"
status = "done"
milestone = "M7"
module = "benchmark"
size = "L"
priority = "P1"
updated = "2026-10-21"
completed = "2026-10-21"
depends_on = ["M7-003"]
docs = ["benchmark-plan", "benchmark-baseline", "adr-0029"]
+++

## 范围与验收

固定正式配置后，在 300 元预算内跑完整配对，公开逐题结果、分母、token/费用、耗时与失败原因；外部基准单独记录。

- [x] 范围已实现，并有独立验收证据。
- [x] 完整仓库检查与 dashboard 同步。

实测：96/96 正式运行完成，分母/哈希/独立证据审计通过；两次真实失败保留。
费用上界 ¥14.219348（含 34 次开发尝试），缺失 usage 为零。详见 benchmark-baseline。
