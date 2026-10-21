+++
id = "terminal-acceptance"
title = "M6 终端验收记录"
kind = "doc"
status = "living"
order = 34
summary = "区分离线回归、PTY 协议测试与 Linux/macOS 实机证据；未验证项不关闭 Ghostty gate。"
+++

## 本次证据

2026-10-21，Linux 6.17.0-40-generic，rustc 1.96.0；基线 commit `029c51a` 加当前 M6 工作区修改。当前执行环境没有 DISPLAY / WAYLAND_DISPLAY。本记录不是已发布版本的实机认证。

| 层次 | 覆盖 | 结果 |
|---|---|---|
| 编辑器单元测试 | 字素、粘贴、撤销、历史、光标 token 补全、ignore、确定性随机编辑序列 | 本地通过 |
| 运行时 / 工具测试 | provider 等待中的输入、去重/FIFO、审批 request ID、背压取消、断开、日志失败、外部修改冲突、退出状态、进程组清理 | 本地通过 |
| 工作区证据测试 | staged/dirty/untracked 基线、index 不变、未知归属、过期检查、binary/大文件/非 Git、日志重建 | 本地通过 |
| Python PTY + Rust 集成入口 | linear/inline × 审批允许/取消，设置菜单一次确认、焦点/光标恢复、粘贴撤销、resize 与按键交错、详情、控制序列清洗、termios 恢复 | 本地通过 |
| CI 配置 | Ubuntu + macOS fmt / clippy / workspace tests / dashboard check；Unix PTY 使用 Python 3 | 已配置，未在本次会话运行远端 CI |
| 真实终端 / IME / 复制 / 字体 / scrollback | 下表全部组合 | 未验证 |
| 长时间慢终端 / 大日志 / 网络文件系统压力 | 设备阻塞、内存和取消延迟测量 | 合成 100 轮 / 30 分钟已通过；真实设备及网络文件系统未验证 |

PTY 用控制序列应答光标位置查询，检查实际日志、文件副作用与 OS 终端属性；不把应答值当作屏幕渲染证据。回归曾复现 resize 与输入交错时的丢键；统一 crossterm 版本、level-triggered 输入后端及有界正超时轮询后通过。详情保留换行，无变化的帧不重复刷屏。

此前 M6 基础实现检查：`cargo test --workspace` 185 passed / 0 failed / 0 ignored；fmt、clippy `-D warnings` 与 dashboard 一致性检查通过。依赖为 crossterm 0.29.0 / ratatui 0.30.2。

## 共享文本布局回归 · 2026-10-21

M6-010 将输入、流式预览、inline transcript、详情、审批 diff 的字素布局集中到
`text_layout`；删除三处只为显示而创建的 Composer，以及独立逐字符换行/裁剪路径。
linear transcript 继续使用终端原生软换行，不修改模式与生命周期。

新增回归覆盖满行/空行/结尾换行、0/1 列宽、中文/组合字符/ZWJ emoji、tab 的逻辑列宽
与满行光标位置、控制字符、超大逻辑行的前缀/提示/后续诊断、diff gutter 中的完整字素。
详情测试从真实 session JSONL 读取并 resize，确认内容重排且日志字节不变。
既有 PTY 测试继续验证 linear/inline × 允许/取消、详情切换、resize 和 termios 恢复。

本次完整仓库测试 200 passed / 0 failed / 0 ignored；fmt、clippy 与 dashboard check 通过。
上述证据不关闭下方实机与长时间压力 gate。

## herdr 定向回归 · 2026-10-21

本机 `herdr --version` / `herdr status` 确认客户端与后台服务均为 0.8.2（protocol 20）。在独立命名会话 `niucode-cursor-check` 中运行离线 replay，未向用户已有工作窗格写入输入。

- inline：`/model` → Enter → Down → Enter，打开不改配置，确认后记录一次 `model_change` 并返回空输入；`/permissions` 同样直接应用。
- 修复前 `pane read --source visible --format text` 复现命令输出从右侧开始：`model` 的 `m` 挤在上一行末尾。ratatui 0.30.2 的 `clear()` 恢复原光标，`insert_before()` 后该位置可在 transcript 行尾。修复后统一交还 viewport 起点，命令结果与输入按行显示；TestBackend 覆盖保存 transcript 和光标起点。
- 独立 herdr 客户端连接 100×34 PTY：菜单最后 cursor mode 为 hidden；确认后为 shown，并转发 `CSI 6 SP q` 稳定竖线光标。未新增 herdr 环境变量判断或品牌分支。
- linear release：`/permissions` → Enter → Down → Enter，正确应用 review 并返回输入；命令结果保持正常换行。
- release 二进制通过 PTY 回归并已更新到 `~/.local/bin/niucode`，与 `target/release/niucode` 的 SHA-256 一致。旧可执行文件构建时间为 09:38，此前工作区修改不会自动生效。独立 herdr 测试会话已关闭并删除。

上述为真实 herdr 服务端快照和客户端控制序列证据，未覆盖 GUI 字体、实际闪烁感受、IME 或长时间压力，不能关闭下方 Ghostty 实机 gate。

## 第二阶段可用性与高亮 · 2026-10-21

M6-011 至 M6-016 的回归、100 轮日志/取消计时、独立 tmux 屏幕捕获与持续输出测试见
[TUI 第二阶段验证](tui-upgrade-validation.md)。该记录区分任务结束与输入框恢复时延。
原始 hello 刷屏会话通过只读副本重放验证；原 benchmark 证据未修改。

## 实机矩阵

| OS | 终端 | 直连 | tmux |
|---|---|---|---|
| Linux | Ghostty | 未验证 | 未验证 |
| Linux | Kitty | 未验证 | 未验证 |
| Linux | WezTerm | 未验证 | 未验证 |
| Linux | GNOME Terminal | 未验证 | 未验证 |
| macOS | Ghostty | 未验证 | 未验证 |
| macOS | Kitty | 未验证 | 未验证 |
| macOS | WezTerm | 未验证 | 未验证 |
| macOS | Terminal.app | 未验证 | 未验证 |

每个组合记录 OS/终端/tmux 版本、字体、配置、窗口尺寸、commit、操作序列和实际截图/录屏；覆盖中文 IME、组合字符/emoji、多行粘贴、长行、resize、流式输出、Enter 补充、Alt-Enter 排队、取消、审批、详情、自然复制与 scrollback。压力测试还记录负载、持续时间、峰值内存、输入/取消延迟及日志大小。

未取得真实 Ghostty 屏幕证据前，M5-003 保持 review、M5-006 保持 blocked，默认 linear fallback 保留。M6-009 的完成不以 PTY、tmux capture 或 CI 代替实机验证。

## 重现自动化

```sh
cargo test -p niucode --test terminal -- --nocapture
cargo test -p niucode-agent --test runtime
cargo test -p niucode-tools --test mutations
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask dashboard
cargo xtask dashboard --check
```

PTY 脚本位于 `crates/niucode-cli/tests/terminal.py`，只用 Python 标准库，无真实 API 调用。Linux `/dev/full` 和 `/proc` 故障/进程测试有明确平台 cfg；macOS 不会为跑测试自动放宽沙箱策略。
