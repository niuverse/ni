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
- CI 即时打印测试失败，并给测试步骤设置 5 分钟超时，避免一个挂起场景隐藏其他失败。

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

## 验收

- [x] workspace tests、fmt、clippy、benchmark 离线验证与 dashboard check 通过。
- [ ] 修复提交在 GitHub Actions 的 Linux/macOS matrix 全部通过。
