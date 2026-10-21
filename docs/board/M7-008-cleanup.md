+++
id = "M7-008"
title = "Remove proven unused recent code and review architecture"
status = "done"
milestone = "M7"
module = "benchmark"
size = "S"
priority = "P2"
updated = "2026-10-21"
completed = "2026-10-21"
depends_on = ["M7-004"]
docs = ["adr-0028", "adr-0029"]
+++

## 范围

用户授权清理无效代码。以 M6 终端与 M7 评测最近改动为范围，先证明消费者再删除。

- [x] 删除 LineEditor::read_line 未使用的 styled 参数和唯一调用处的格式化分支。
- [x] 删除 adapter.py 的 subprocess 与 live.py 的 shutil 未使用 import。
- [x] 架构审查区分可直接删除与需要设计的机会；报告保存在 OS 临时目录。
- [x] workspace tests、fmt、clippy、Python benchmark tests、dashboard 通过。

当时的两个后续候选已分别由 M6-010（共享文本布局）和 M6-013（详情/审批状态）落实。
没有为了减行删除 baseline、fixture、锁文件、
费用计量或取消/恢复逻辑；这些都有实际消费者。终端实机 gate 保持开放。

2026-10-21：用户选择统一文本布局，后续实现与回归由 M6-010 跟踪。
