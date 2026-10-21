+++
id = "M6-017"
title = "Remove startup residue and consolidate PTY test entrypoints"
status = "done"
milestone = "M6"
module = "tui"
size = "S"
priority = "P2"
updated = "2026-10-21"
completed = "2026-10-21"
depends_on = ["M6-013", "M6-016"]
docs = ["adr-0028", "adr-0031"]
+++

## 范围

按实际消费者清理近期实现，并将积累的改动按依赖关系提交。
不更改事件格式、benchmark 评分规则或已有证据。

- 删除 TUI 已不显示的凭证来源查询，保留运行时模型切换所需的 factory。
- 删除 headless 未使用的 factory 返回值与重复上下文预算推导；保留其凭证来源诊断。
- 三个独立 PTY 回归共用 Python 启动和失败输出逻辑，保留原场景和测试名称。
- 保留长时间压力脚本、合成题错误补丁、dev smoke 与冻结证据；它们都有真实验收消费者。

## 验收

- [x] workspace tests、fmt、clippy、benchmark 离线自检与 dashboard check 通过。
- [x] 原有 58 个 benchmark evidence 文件逐字节一致。
- [x] 代码、评测设施与文档分批提交，无临时报告或凭证入库。

仅删除和收敛既有实现，不改变 ADR-0028/0031 的架构契约。
真实 Linux/macOS/Ghostty 终端验收门槛保持原状态。

本次清理净减少 31 行代码，未新增依赖或公共接口。完整回归重跑通过，但 debug 多轮
测试暴露既有间歇性崩溃；清理前实现也复现，由 M6-018 跟踪，不包含在“清理完成”结论内。
