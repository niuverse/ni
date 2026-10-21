+++
id = "M7-005"
title = "Harness reliability scenarios beyond coding success"
status = "ready"
milestone = "M7"
module = "benchmark"
size = "L"
priority = "P1"
updated = "2026-10-21"
depends_on = ["M7-003"]
docs = ["benchmark-plan", "adr-0029"]
+++

## 范围与验收

后续阶段规划，不包含在首轮 96 次编码成绩中。小题成功率无法证明 harness 的恢复、
取消、上下文管理和写冲突能力；这些行为需单独构造确定性场景。

- [ ] 固定故障注入：429/流中断、工具超时、输出截断、取消后子进程清理。
- [ ] 固定状态场景：重启恢复不重复副作用、外部修改导致 stale write、长历史压缩保留约束。
- [ ] 每个场景公开输入、可观察不变量及失败证据；不采用模型自评。
- [ ] 各 harness 能力不支持、接入错误与行为失败分列；不能实现公平对齐的场景不生成排名。
- [ ] 费用/取消路径复用现有网关与 runner，不扩大 core，不新增持久会话状态。
- [ ] 与 M6 实机终端矩阵分别报告，自动化不关闭 Ghostty gate。
