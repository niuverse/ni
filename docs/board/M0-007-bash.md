+++
id = "M0-007"
title = "Bash 工具：超时、进程组与输出预算"
status = "backlog"
milestone = "M0"
module = "sandbox"
size = "M"
priority = "P0"
updated = "2026-10-20"
depends_on = ["M0-006"]
+++

## 范围

- 在工作目录执行命令，默认超时 120s。
- 启动时 `detached`，取消/超时对进程组发信号，不留孤儿。
- 只回尾部 20KB，完整输出写临时文件并把路径给模型。
- 变更型命令先过 approval 策略（M0 可以是占位实现）。

## 验收

- 超时后子进程及其派生的 `sleep` 都被终止。
- 大输出不会撑爆上下文，模型能拿到完整输出路径。
- Ctrl-C 能中断正在跑的命令。
- 非零退出码、stderr 和超时是可区分的三种结果。

## 记录

bash 是能力上限也是风险上限。没有真沙箱时，所有变更型 bash 强制审批。
