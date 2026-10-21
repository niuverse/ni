+++
id = "M6-001"
title = "Terminal pairing contract and implementation plan"
status = "done"
milestone = "M6"
module = "tui"
size = "M"
priority = "P1"
updated = "2026-10-21"
completed = "2026-10-21"
depends_on = []
docs = ["pairing-upgrade", "adr-0028"]
+++

## 范围

- 落实用户确认的完整日常结对范围，明确模块所有权、迁移顺序、失败语义和平台验收。
- 本卡只交付设计与实施计划，不代表任何新增运行行为已完成。

## 验收

- [x] 用户确认原生 scrollback、运行中补充、收起 reasoning、累计 diff、共享工作区及 Linux/macOS 范围。
- [x] ADR-0028 记录决策，明确 ADR-0021 的替代时机与 ADR-0022 的保留门槛。
- [x] 实施计划包含 runtime/日志边界、冲突保证范围、审阅证据与代码质量约束。
- [x] 实现卡有依赖和可执行验收，实机矩阵保持未验证；看板源与生成物同步。

## 记录

2026-10-21：完成共识与实施计划；运行时改动由后续实现卡交付。

实现须遵守 pairing-upgrade 中的 robust/clean 审查门槛及仓库 Definition of done。
