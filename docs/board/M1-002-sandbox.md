+++
id = "M1-002"
title = "Sandbox 三轴与原生隔离"
status = "done"
milestone = "M1"
module = "sandbox"
size = "L"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M0-007"]
docs = ["adr-0006", "exec"]
+++

## 范围

- `sandbox`：read-only / workspace-write / full-access。
- `approval`：ask / on-failure / never。
- `network`：on / off。
- Linux 第一后端用 bwrap namespace；landlock/seccomp 与 macOS seatbelt 后续补。
- 路径策略与 shell 隔离分开实现，不假装进程内检查是安全边界。

## 验收

- [x] workspace-write 下无法写工作区外路径（bwrap 只读根 + cwd bind）。
- [x] 网络关闭时子进程连接被拒绝（network namespace）。
- [x] 逃逸尝试进入 tool_result / policy / approval 事件，不静默失败。
- [x] 无 bwrap 时 `never` 启动失败，`on-failure`/`ask` 降级到逐工具审批并提示网络策略未执行。
- [x] `read-only` 同时拒绝文件工具 write/edit 和 bash 写入。
- [x] `full-access + network=on` 是显式 opt-out，默认不可达。

## 记录

安全模型一句话：默认只能写 cwd、不能联网；要问人时逐工具问；full-access 必须显式选择。实现决策见 ADR-0006。
