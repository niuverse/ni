+++
id = "M6-019"
title = "Restore Linux and macOS CI with explicit sandbox prerequisites"
status = "review"
milestone = "M6"
module = "extensions"
size = "S"
priority = "P1"
updated = "2026-10-21"
depends_on = ["M4-007"]
docs = ["adr-0025"]
+++

## 问题

[CI run 39576343751](https://github.com/niuverse/niucode/actions/runs/39576343751)
在 Linux 和 macOS 的 `exec_mounts_calls_and_unmounts_a_sidecar` 中失败。
测试无条件要求 bwrap 挂载成功，但 Linux job 未安装 bubblewrap，macOS 不支持
该 Linux namespace 后端。将本地测试进程的 PATH 指向空目录可复现相同断言失败；
实际日志记录 `extension_error`，原因是无法启动 bwrap。

## 修改

- Linux CI 安装 bubblewrap，并在测试前实际创建隔离 namespace；依赖不满足时明确失败。
- 仅在一次性 Ubuntu CI VM 中关闭 AppArmor 的 unprivileged userns 限制，使 bwrap
  能配置 namespace；不关闭 bwrap 的网络/文件隔离，不修改开发机或产品行为。
- 真实 sidecar 挂载、网络/文件隔离、worker pool 测试只在 Linux 执行，保留全部断言。
- 所有平台运行缺少 bwrap 的 CLI 回归：子进程使用空 PATH，必须记录挂载错误，
  不得注册/卸载未挂载扩展，其工具调用必须失败。不会修改测试宿主的 PATH。
- 挂载成功断言附上 stderr 与会话日志，便于定位后续环境问题。
- CLI、agent 与 subagent 的真实 sidecar 场景都限定到 Linux；其余 agent 回归保留跨平台执行。
- PTY 测试使用独立会话中的 stdin 终端，不调用 `TIOCSCTTY`，以便 macOS 在子进程退出后
  仍可读取并核对恢复的 termios。异常清理先关闭 PTY，再有界等待回收子进程。
- CI 即时打印测试失败，继续执行其他测试目标，并给测试步骤设置 5 分钟超时，避免一个挂起场景隐藏其他失败。
- 批量写入 fixture 显式采用 `full-access / network on / approval never`，不依赖 bwrap 执行断网。
- Toolbox 构造时统一 canonicalize 只读 roots，避免 macOS `/var` 与 `/private/var` 的别名
  导致授权目录被误拒；符号链接根目录回归同时检查只读、改向后拒绝和原根目录仍可读。
- 子代理隔离断言验证独立 session ID/路径、唯一任务消息和匹配回复，不把快照正文中的
  其他任务名误判为串日志；fixture 主动包含两项任务名，确保该情形稳定覆盖。

ADR-0025 的强制隔离不变；不提供无沙箱降级，不修改 benchmark 冻结输入或证据。
CI 的 macOS PTY 回归仍保留，不代替真实终端验收。

首次补齐依赖后，[run 39577692305](https://github.com/niuverse/niucode/actions/runs/39577692305)
的 Linux 预检暴露 `bwrap: loopback: Failed RTM_NEWADDR: Operation not permitted`。
Ubuntu 的默认 AppArmor 策略允许创建 user namespace，却限制其中的 capabilities，
见 [Ubuntu 24.04 release notes](https://documentation.ubuntu.com/release-notes/24.04/)
和 [Ubuntu 的 userns 配置说明](https://ubuntu.com/blog/ubuntu-23-10-restricted-unprivileged-user-namespaces)。
CI 配置采用仅影响当前 VM 的 sysctl，不写系统持久配置。

随后 [run 39578381513](https://github.com/niuverse/niucode/actions/runs/39578381513)
暴露 macOS PTY 测试壳的问题：退出后 `tcgetattr(slave)` 返回 ENOTTY，volume 场景
异常清理停在 `Terminal.close()` 的 `child.wait()`。临时 faulthandler 取得调用栈后移除，
保留原有终端属性恢复、取消、详情和输出量断言。Linux 同轮全部检查通过。

[run 39579150782](https://github.com/niuverse/niucode/actions/runs/39579150782)
确认恢复后的 termios 仅 `c_lflag` 从 1483 变为 536872395，差值为 `PENDIN`（0x20000000）。
[XNU tty.c](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/tty.c)
在重新启用 ICANON 时主动设置该 pending-input 状态位，并在下一次输入处理中清除。
常规 PTY 和压力脚本共用恢复断言：macOS 仅排除此内核状态位，所有模式、速率和控制字符
仍须一致；其他平台继续完整比较。不改生产代码来清除一个正常的内核状态。

[run 39579470906](https://github.com/niuverse/niucode/actions/runs/39579470906)
的 macOS PTY 已全部通过，完整 Rust 测试继续暴露上述 roots 匹配和日志断言问题。
两者均在 Linux 新 fixture 下先复现失败、修复后通过，不只依赖 macOS runner 验证。

## 验收

- [x] workspace tests、fmt、clippy、benchmark 离线验证与 dashboard check 通过。
- [ ] 修复提交在 GitHub Actions 的 Linux/macOS matrix 全部通过。
