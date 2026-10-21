+++
id = "M6-007"
title = "Observed file versions and explicit write conflicts"
status = "review"
milestone = "M6"
module = "tools"
size = "L"
priority = "P1"
updated = "2026-10-21"
depends_on = ["M6-001"]
docs = ["pairing-upgrade", "adr-0028"]
+++

## 范围

- 将路径观测升级为内容版本观测；统一 prepare/validate/apply 与 preview，保留 W1 串行策略。
- 文件工具采用乐观冲突检测；明确任意 shell、扩展与外部编辑的残余竞争边界。

## 验收

- [x] 等长/同 mtime 外部修改、删除重建、symlink 目标变化不会被路径已读标记掩盖。
- [x] 预览后内容变化使审批失效；重新读取和展示后才能应用新版本。
- [x] 部分读取与整文件覆盖的授权边界有测试；新文件竞争创建不静默覆盖。
- [x] 写失败不静默截断旧文件；替换过程验证权限/可执行位、清理与失败恢复。
- [x] 内部 Worker 写入仍串行；外部未知变化不自动回滚，不声称 CAS/事务隔离。
- [x] 冲突通过结构化结果传给 runtime/UI，不能靠解析错误字符串推断。
- [x] 公共 Toolbox/preview 入口覆盖成功、冲突和错误路径，删除旧 observed-only 检查。

## 记录

2026-10-21：prepare/preview/apply 共用不可变版本，W1 在审批后获取；临时文件替换且保留权限，结构化 conflict 与进程 evidence。测试覆盖同长度同 mtime 外改、替换文件、symlink 换目标、审批后变化、部分读取、新文件竞争和可执行位。

实现须遵守 pairing-upgrade 中的 robust/clean 审查门槛及仓库 Definition of done。


验证索引：`docs/terminal-acceptance.md`。review 表示代码与本地回归可审阅，未宣称全部平台验收完成。
