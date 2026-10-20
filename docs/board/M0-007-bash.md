+++
id = "M0-007"
title = "Bash 工具：超时、进程组与输出预算"
status = "done"
milestone = "M0"
module = "tools"
size = "M"
priority = "P0"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M0-006"]
docs = ["exec"]
+++

## 范围

- 在工作目录执行命令，默认超时 120s。
- 启动时 `detached`，取消/超时对进程组发信号，不留孤儿。
- 只回尾部 20KB，完整输出写临时文件并把路径给模型。
- approval 推迟到 M1-002；M0 只保证进程组、超时和输出预算。

## 验收

- [x] 超时后子进程及其派生的 `sleep` 都被终止（独立进程组 + 组信号）。
- [x] 大输出不会撑爆上下文，模型能拿到临时输出路径（最多保留 8MB 捕获）。
- [x] Ctrl-C 能中断正在跑的命令并杀掉进程组。
- [x] 非零退出码、stderr 和超时是可区分的三种结果。

## 记录

bash 是能力上限也是风险上限。M0 只做进程组/超时/输出预算；真正的 sandbox 和 approval 在 M1-002。
