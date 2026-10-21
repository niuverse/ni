+++
id = "M6-008"
title = "Task-scoped changes and evidence-based verification"
status = "review"
milestone = "M6"
module = "loop"
size = "L"
priority = "P1"
updated = "2026-10-21"
depends_on = ["M6-004","M6-006","M6-007"]
docs = ["pairing-upgrade", "adr-0028"]
+++

## 范围

- 采集本轮工作区基线与操作证据，生成累计 diff 与完成汇总；结构化记录进程退出、超时和取消。
- 基线不以 HEAD 代替，不写用户 index/stash；受预算约束的持久审阅事实进入日志。

## 验收

- [x] 已有 staged/unstaged/untracked 内容保留，累计 diff 以任务开始磁盘状态计算。
- [ ] 反复编辑、取消、非 Git、二进制、大文件、扫描期间修改与覆盖预算超限均有明确结果。
- [x] shell/扩展或外部修改归属不明时单独标注；未知变化不计作 agent 成果。
- [x] 退出 0、非零、信号、超时、取消、调用错误区分记录，旧 ToolResult.ok 不静默改义。
- [x] 验证项关联具体命令/cwd/call ID；任意命令退出 0 不冒充整体测试通过。
- [x] 验证后继续修改时标记结果可能过期；未执行检查、evaluator met 不显示测试通过。
- [x] 删除派生缓存后仍能从日志恢复已记录审阅事实；超限未记录内容明确不可重建。
- [x] 日志写入失败和副作用后中断不伪造成功；恢复不会自动重放修改。

## 记录

2026-10-21 用户试用发现：hello 无工具调用时，快照预算外的 130 个无 digest 条目被误报为工作区变化。
该缺陷由 M6-011 修复并补“不完整前后快照相同”的回归，采集开销由 M6-014 跟踪。
M6-011 已完成该语义修复和端到端回归，M6-014 已完成正文复用；本卡的原实机范围继续保持 review。

2026-10-21：任务前后受预算快照、累计 diff 与命令证据已实现；dirty/staged/untracked/index 保留、未知归属、过期检查、非 Git/binary/大文件和日志重建有测试。缺失证据不重放工具；实际设备失败与长时间负载还需验收。

实现须遵守 pairing-upgrade 中的 robust/clean 审查门槛及仓库 Definition of done。


验证索引：`docs/terminal-acceptance.md`。review 表示代码与本地回归可审阅，未宣称全部平台验收完成。
