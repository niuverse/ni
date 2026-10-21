+++
id = "M6-006"
title = "Readable transcript and event-backed detail views"
status = "review"
milestone = "M6"
module = "tui"
size = "L"
priority = "P1"
updated = "2026-10-21"
depends_on = ["M6-003","M6-004"]
docs = ["pairing-upgrade", "adr-0028"]
+++

## 范围

- 默认收起 reasoning，显示真实阶段/耗时；工具简报、代码和 diff 层次清晰。
- 按任务/call ID 查看已记录 reasoning 和任意历史工具结果，替代只能查看最近输出的限制。

## 验收

- [x] 主屏默认不流式刷出 reasoning，详情可访问全部已记录内容；中断未记录部分明确不可恢复。
- [x] 历史详情来源是日志投影，resume 后仍可定位；不依赖当前进程 last_tool 缓存。
- [x] 详情内的运行事件有界接收，退出后 transcript 有序补齐，输入草稿与光标恢复。
- [x] 详情打开时取消仍有效；批准/拒绝仅在明确审批状态生效。
- [x] 标题、列表、代码块、失败摘要、长行和控制字符有确定性呈现测试。
- [x] 完成默认行为切换时更新 ADR-0021 和 tui 文档；不提前移除 Ghostty 验收 gate。

## 记录

2026-10-21 用户试用发现：完成时无界打印审阅清单，且 live/tui 重复输出完成信息。
M6-012 跟踪主屏输出预算与紧凑呈现，M6-013 收敛重复呈现责任，M6-015 补输出规模验收。
M6-012/M6-013/M6-015 已修复并验证长清单下的主屏预算；本卡的原实机范围继续保持 review。

2026-10-21：默认收起 reasoning；Ctrl-O 和 /last 从 JSONL 定位历史与累计 diff，详情有界暂存运行输出。PTY 检查隐藏/查看、控制序列、模式恢复；ADR-0021 默认策略已更新。

实现须遵守 pairing-upgrade 中的 robust/clean 审查门槛及仓库 Definition of done。


验证索引：`docs/terminal-acceptance.md`。review 表示代码与本地回归可审阅，未宣称全部平台验收完成。
