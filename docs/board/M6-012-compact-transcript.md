+++
id = "M6-012"
title = "Bound transcript noise and emit one task completion"
status = "done"
milestone = "M6"
module = "tui"
size = "M"
priority = "P0"
updated = "2026-10-21"
completed = "2026-10-21"
depends_on = ["M6-011"]
docs = ["pairing-upgrade", "tui", "adr-0028", "adr-0031", "tui-upgrade-validation"]
+++

## 范围

主屏保留回答、紧凑进度及必要异常；详细文件清单、命令和审阅证据由 Ctrl-O 展示。
移除 live/tui 重复完成信息，按实际副作用与证据决定验证提示，不用关键词或模型分类对话。
启动、运行和结束使用 pairing-upgrade 第二阶段的信息预算。保留默认 linear/native scrollback。

## 验收

- [x] 无工具 hello 只出现一次完成信息，不输出文件清单或检查缺失警告；覆盖不足保留在详情。
- [x] 80 列启动至多 2 显示行；每任务系统收尾至多 3 行，20/40 列至多 5 行，与路径/命令长度、文件数无关。
- [x] 连续成功工具调用合并进度，刷新不逐次追加历史；回答及代码正文不套用系统摘要的行数预算。
- [x] 真实变化后无检查、检查失败/过期、未知覆盖分别显示；任意命令成功不称整体验收通过。
- [x] 失败、取消、生成截断、冲突、日志错误不报正常完成；审批保留具体操作与 diff。
- [x] Ctrl-O 可访问完整已记录证据；恢复后仍可定位任务及调用，底层截断明确可见。
- [x] PTY 覆盖 hello、百文件改动、超长命令、失败与恢复；workspace tests、fmt、clippy、dashboard check 通过。

首次可试用交付与 M6-011 一起验收，记录构建产物 hash 和 PATH 实际入口。

## 实施记录

2026-10-21：实现与回归通过，详见 `docs/tui-upgrade-validation.md`。真实平台门槛继续由 M6-009 跟踪。
