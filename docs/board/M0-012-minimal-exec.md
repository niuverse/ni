+++
id = "M0-012"
title = "最小 ni exec 闭环"
status = "done"
milestone = "M0"
module = "loop"
size = "L"
priority = "P0"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M0-005"]
docs = ["exec", "architecture"]
+++

## 范围

- 新增 `ni-tools`：`read` / `write` / `edit` / `bash`，路径限制、read-before-write、输出截断。
- 新增 `ni-agent`：turn loop、流式文本、tool call 聚合、工具派发、事件落盘、max steps。
- `ni exec "<task>"`：通过 `ni-auth` 取 key，创建 session，流式输出。
- Session 事件：`session_start → user_message → assistant_message → turn_end → tool_result* → … → session_end`。

## 验收

- [x] 真实 DeepSeek `deepseek-flash` 文本响应成功。
- [x] 真实 DeepSeek tool call（read）成功，第二轮返回内容。
- [x] 本地 mock SSE 覆盖 text 与 tool 两条路径，不消耗 token。
- [x] 脚本化 provider 测试覆盖无工具完成、工具后继续、max steps。
- [x] 文件工具拒绝工作区外路径；已有文件必须先 read。
- [x] usage / cached tokens 落在 `turn_end`。

## 记录

后续 M0-006/007/008 已补齐 grep/glob、进程组超时和取消。这是最小闭环，不是安全闭环：`bash` 可执行任意命令，没有 approval/sandbox；`grep`/`glob`、进程组 kill、resume、compaction 仍未完成。真实 DeepSeek 测试在临时工作区执行，session 已清理。
