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

ADR-0025 的强制隔离不变；不提供无沙箱降级，不修改 benchmark 冻结输入或证据。
CI 的 macOS PTY 回归仍保留，不代替真实终端验收。

首次补齐依赖后，[run 39577692305](https://github.com/niuverse/niucode/actions/runs/39577692305)
的 Linux 预检暴露 `bwrap: loopback: Failed RTM_NEWADDR: Operation not permitted`。
Ubuntu 的默认 AppArmor 策略允许创建 user namespace，却限制其中的 capabilities，
见 [Ubuntu 24.04 release notes](https://documentation.ubuntu.com/release-notes/24.04/)
和 [Ubuntu 的 userns 配置说明](https://ubuntu.com/blog/ubuntu-23-10-restricted-unprivileged-user-namespaces)。
CI 配置采用仅影响当前 VM 的 sysctl，不写系统持久配置。

## 验收

- [x] workspace tests、fmt、clippy、benchmark 离线验证与 dashboard check 通过。
- [ ] 修复提交在 GitHub Actions 的 Linux/macOS matrix 全部通过。
