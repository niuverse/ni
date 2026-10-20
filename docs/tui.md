+++
id = "tui"
title = "Inline TUI、Diff Pager 与审批"
kind = "doc"
status = "living"
order = 33
summary = "niucode tui：ratatui inline 输入层、原生 transcript scrollback、审批 diff pager、工具状态与缓存命中率。"
+++

## 用法

```sh
niucode                       # 直接进入 TUI
niucode tui
niucode tui --cwd /path/to/repo
niucode tui --sandbox read-only --network off
niucode tui --approval ask
niucode tui --resume /path/to/session.jsonl
niucode tui --replay /path/to/fixture.json
niucode tui --context-budget 128000 --keep-recent-turns 4
niucode tui --max-steps 40
```

默认策略与 `niucode exec` 相同：`workspace-write + network off + on-failure`。

## 交互

```text
███╗   ██╗██╗██╗   ██╗ ██████╗ ██████╗ ██████╗ ███████╗
...
minimal · observable · event-sourced · v0.1.0
inline input · native scrollback · :help commands · Ctrl-C cancels the turn

niucode ▸ find the failing test
» bash {"command":"cargo test"}
✓ bash exit: 1
│ 12 lines · 1.8 KB · :last to expand
╰─ turn 1 · in 1842 · out 96 · cache 42%
◆ I found the failing case…
╰─ turn 2 · in 2104 · out 120 · cache 61%
✓ done · 2 turns · in 3946 · out 216 · cache 52%

niucode ▸ :last
╭─ last tool · bash · exit: 1
│ ...
╰─ end

niucode ▸ exit
```

- 输入一行任务，Enter 提交；`exit` / `quit` / `/exit` / `:q` 退出。
- 执行 turn 时 `Ctrl-C` 取消当前 turn；2 秒内再次 `Ctrl-C` 强制退出。输入行时 `Ctrl-C` 清空当前行，`Ctrl-D` 退出。
- `Tab` 补全 slash 命令；多个匹配时显示带说明的候选列表。唯一前缀会显示灰色内联提示。
- 菜单关闭时，`↑` / `↓` 浏览当前进程内的输入历史；`Ctrl-R` 用当前输入做反向历史搜索，重复按继续向前找。
- `Ctrl-N` / `Ctrl-P` 等同 `↓` / `↑`；`Ctrl-Left/Right` 或 `Alt-B/F` 按词移动；`Ctrl-L` 清屏；支持 bracketed paste。
- 默认是 ratatui inline 输入层（picker/help、3 行输入、footer）；`NIUCODE_TUI_LINEAR=1` 只在终端不支持 cursor position 时作为 fallback。
- 鼠标捕获只在 picker 打开时启用，hover/click 用绝对屏幕坐标命中；菜单关闭后终端恢复原生复制/拖拽。退出、panic 或 Ctrl-C 都会经 `TerminalGuard` 释放 raw mode / bracketed paste / mouse capture。
- 启动时主动关闭残留的 mouse reporting / bracketed paste / alternate screen，修复上一次异常退出后“终端卡住、无法复制”的状态。
- 非 TTY（管道/脚本）自动退化为逐行输入，不启用行编辑。
- `◆` 标记 assistant 文本，`◇` 标记折叠后的 reasoning，`»` 标记工具调用。
- `subagent` 调用会显示任务输入，完成后显示每个子任务的摘要和独立日志路径；父上下文只收到这一条 tool_result。
- 工具结果显示 `✓` / `✗`，短输出用 `│` 缩进；超过 12 行或 2 KB 时折叠为摘要和 `:last to expand`。
- `╰─ turn N` 在该 turn 的工具结果结束后打印，包含 input / output / cache 百分比。
- `--max-steps` 控制每个用户请求的最大模型 turn 数；默认 `0` 表示不设用户级上限（内部 256 安全 cap）。显式设置时达到上限会打印 `max steps reached`，已写入的项目文件保留。
- `⇣ compacted · N events · ~before → ~after tokens` 表示上下文压缩；失败显示 `! warning: compaction skipped: ...` 并保留原上下文。
- `:last` 重新打印最近一次工具输出；`:help` 显示可用命令。展开只保留在当前进程内，完整输出始终在 session JSONL 里。
- 文本增量直接写到 stdout，首 token 到达即渲染。

## 行编辑与补全

TTY 下使用 readline 风格的行编辑器：

- 输入 `/` 后自动显示 slash 命令和说明；输入 `/i` 直接出现 `/init` 候选，不需要先按 Tab。
- `Tab` / `Enter` 接受当前候选；`↑` / `↓` 选择；`Esc` 关闭菜单。
- 鼠标移动到候选行会高亮；左键点击接受；滚轮在菜单内移动选择。
- 终端支持 cursor-position 时按绝对行号 hover；不支持时退化为相对移动 hover（第一次移动作为锚点，后续按行差改变候选），键盘操作始终可用。
- tmux 用户需要 `tmux set -g mouse on`；TUI 启动会检测 `TMUX` 并提示。终端完全不发送 motion 事件时 hover 不可用，但 click/键盘仍可操作。
- 输入的命令名用 accent 高亮；prompt 会显示 `plan` / `goal` 徽标。
- 命令继续输入参数（例如 `/goal all tests pass`）后菜单自动关闭。
- `↑` / `↓` 调出当前进程内的历史输入；`Ctrl-C` 清空当前行；`Ctrl-D` 退出；`Ctrl-L` 清屏。
- 超长输入、光标移动、退格、删除、Home/End 由行编辑器处理，不再依赖终端自身回显。
- 非 TTY 时走原来的逐行 `read_line`，方便管道和测试。
- 参数补全覆盖 `/model`（模型）和 `/permissions`（profile）；`/fork` 的 event ID 仍需直接输入。

## Plan mode 与 Goal

- `/plan` 只收紧权限：`effective_access = min(base_access, read-only)`，写/命令/联网被阻止或询问；它不改变当前 sandbox/approval 配置，退出后恢复。
- `/plan` 生成的计划就是该 turn 的 assistant 输出；`/plan approve` 只退出模式，不自动执行，下一次 prompt 才进入实现。
- `/goal <condition>` 现在会**设置目标并立即启动** goal run；如果只想设置不执行，可在 `/plan` 只读模式下设置（此时只存目标，不启动 run）。
- 每轮结束调用独立 evaluator（无工具、30s timeout），verdict 写入 `goal_verdict`；met 自动清除 goal，impossible/6 轮预算耗尽停止。`/goal stop` 停止但不删 goal。headless 版本在 M4-008。
- 启动时可用 `--plan` 直接进入 plan mode；profile 的 CLI/`/permissions` 随 M4-003 落地，当前仍用 `--sandbox` / `--approval`。

## Slash 命令

输入 `/` 开头的本地命令；不以 `/` 开头的文本照常发给模型。`:` 保留 `:help` / `:last` / `:q` 三个兼容别名。

| 命令 | 行为 |
|---|---|
| `/init` | 在当前工作区创建 `AGENTS.md` scaffold；已存在则不覆盖，read-only 沙箱拒绝。 |
| `/clear` | 新建对话 session；保留 plan mode 和 goal，清空上下文；旧 session 文件保留。 |
| `/reset` | 新建对话 session，并清除 plan/goal/pending/输入历史，重新读取 AGENTS.md；旧 session 保留。 |
| `/resume [path|id]` | 无参数列出最近 8 个 session；带参数切换并恢复其 history / plan / goal。 |
| `/reload` | 重新读取 AGENTS.md 并显示 sources/digest；下一次 turn 生效。 |
| `/model [name]` | 输入 `/model` 直接打开模型下拉；↑↓/鼠标选择，Enter 填入，再 Enter 执行；只允许当前 provider。 |
| `/compact` | 立即生成确定性 handoff 并写入 compaction 事件；下一次 turn 使用压缩后的投影。 |
| `/permissions [profile]` | 输入 `/permissions` 直接打开 profile 下拉；切换 ask/review/full-access/ci。 |
| `/plan` | 进入只读 plan mode；`/plan off` / `/plan approve` 退出；`/plan status` 查看。 |
| `/goal <condition>` | 设置目标并立即启动 goal run（最多 6 轮）；`/goal` 查看；`/goal run` 重新启动；`/goal stop` 停止；`/goal clear` 清除。 |
| `/fork [event-id]` | 复制当前 session 的前缀到新 session 并切换过去；默认 fork 到最后一个事件。 |
| `/status` | 显示 model、cwd、sandbox、context budget、session、事件数、turn / compaction / instruction / subagent / extension 计数和累计 cache。 |
| `/session` | 显示当前 session 路径、事件数和 turn 数。 |
| `/last` | 重新打印最近一次工具输出。 |
| `/help` | 显示命令表。 |
| `/exit` | 退出。 |

细节：

- `/init` 根据 `Cargo.toml` / `package.json` / `pyproject.toml` / `go.mod` 选择测试、构建、lint 命令；未知项目保留占位注释。它只写 `<cwd>/AGENTS.md`，不跟随已存在的 symlink。
- `/plan` 不改变 profile，只把 tool allowlist 收窄到 `read/grep/glob` 并注入只读计划指令；`/plan approve` 只退出模式，不自动执行。
- `/goal` 不改变权限；active goal 进入 system prompt，并写入 `goal_set` / `goal_cleared` 事件。
- session 默认写入 `~/.niucode/projects/<key>/sessions/`；项目根不创建 `.niucode`。`/clear` 继承当前 model、cwd 和 sandbox 配置，写入新的 `session_start` / `policy` 事件；保留 plan mode 与 goal（重新写入事件），停止 goal run，保留输入历史。
- `/reset` 是更强的重置：新 session + plan off + goal 清空 + pending/输入历史清空 + 重新读取 AGENTS.md；extension host 仍按 session 生命周期挂载，不重启进程。
- `/resume` 切换 append-only session：重新投影 history 和 UI 状态，停止当前 goal run；无参数列出当前项目最近 10 个 session（标题取首条 user message），`/resume --all` 跨项目；cwd/model 不一致会警告。
- `/reload` 只重读 AGENTS.md 指令；扩展 manifest 在 session 启动时挂载，重新加载需要重启 TUI。
- `/model` 是运行时模型选择：canonical `provider/model`；写入 `model_change { provider, model }`，`/resume` 恢复，`/clear` 保留，`/reset` 清除。provider 由 `ProviderFactory` 惰性解析凭据并构造；切换发生在 turn 边界。请求仍然走同一个 endpoint/auth，下一次 turn 使用新 model；切换 turn 可能失去 prompt cache。
- `/model` 和 `/permissions` 直接打开参数下拉：输入命令后→↑↓/鼠标选择（或 Tab 选第一项）→Enter 填入→再 Enter 执行；直接 Enter 显示当前值，不误改配置。建议列表来自 `NIUCODE_MODEL_CHOICES`（逗号分隔）或内置的 deepseek/openai 小列表；这是 Pi 式“选择 vs 实际请求模型”的简化版，不含 virtual model/router。
- `/permissions` 只映射已有 policy 字段：`ask` = workspace-write/off/ask，`review` = workspace-write/off/on-failure，`full-access` = full-access/on/never，`ci` = read-only/off/never。写入 `permission_change` 事件；`/resume` 恢复，`/clear` 保留，`/reset` 清除。`/plan` 仍然是独立 mode。
- full-access 会打印隔离关闭警告；profile 变更只在下一个 turn 生效。
- `/status` 显示生效 provider/model、sandbox/network/approval、permission profile、auto context budget 和鼠标模式；tmux 下会提示 `tmux set -g mouse on`。
- `/fork` 调用与 `niucode fork` 相同的逻辑：只读源文件、复制前缀、追加 `fork` 溯源事件，然后重新投影 history 并切换过去。非法 event ID 会报错并保留当前 session。
- 未知 `/foo` 显示 `unknown command`，不会发给模型；命令参数错误显示 usage。
- 所有 slash 输出走 stdout，warning / error 走 stderr。

## 视觉与可访问性

- TTY 宽度 ≥ 62 列时显示 6 行块字 `NIUCODE` logo；窄终端退化为单行 wordmark。
- 颜色只在 TTY 开启；`NO_COLOR`、`CLICOLOR=0`、`TERM=dumb` 或重定向输出时自动退化为纯文本。
- 符号使用单列宽的 UTF-8 字符，不依赖 Nerd Font 或外部字体。
- 审批 pager 的 header / footer 反色条也遵循同一套主题开关。
- 所有文件内容、工具输出和路径在进入终端前都会清洗控制字符，避免 ANSI 注入。

## 审批

`--approval ask` 时，每个 `write` / `edit` / `bash` 前暂停。`write` / `edit` 会先展示本次修改的完整 unified diff：

```text
 approval · edit src/main.rs · +1 -1
▎ src/main.rs                                                    +1 -1
@@ -12,3 +12,3 @@
  12  12 │   fn main() {
  13     │ -     println!("old");
      13 │ +     println!("new");
  14  14 │   }

 file 1/1 · 1-7/7   a allow · d deny · s session · j/k scroll · n/p file
```

pager 按键：

- `a` 允许本次。
- `d` / Esc / `q` 拒绝本次。
- `s` 本次 session 内自动允许后续变更型工具。
- `j` / `k` 或方向键逐行滚动；PageUp / PageDown / 空格翻页；`g` / `G` 跳到顶部 / 底部。
- `n` / `p`（或 `]` / `[`）在多个文件之间跳转。
- 终端 resize 会按新宽度重新布局；窄于 40 列或低于 6 行时退回纯文本预览。
- 终端不是 TTY 时退化为逐行 `a/d/s` 输入，方便管道测试。
- `bash` 没有文件 diff，审批只显示命令摘要和按键提示。

diff 使用工具层读取的真实文件内容，包含行号、hunk 和上下文。`write` 新文件显示全部新增行；已有文件显示修改前后的 diff；二进制文件或无法读取的文件会显示说明而不是伪造 diff。预览与真正执行之间文件可能变化，执行时会重新校验。

## 上下文压缩

`niucode tui` 与 `niucode exec` 共用 `niucode-agent` 的预算检查。默认上下文预算 64k，到 75% 时触发 compaction；默认保留最近 3 个 user turn 原文，其余事件生成确定性 handoff。

```sh
niucode tui --context-budget 128000 --keep-recent-turns 4
niucode tui --max-steps 40
```

- `compaction.upto` 是绝对事件下标；最后一个 compaction 生效，原始事件不删除。
- 压缩成功才追加事件；`compacted` 行显示替换的事件数和估算 token 变化。
- 没有安全 cutoff、handoff 不收缩或写入失败时，保留原上下文并提示，不丢历史。
- 压缩结果会在下一次请求和 resume 时投影为 system handoff，与压缩时的模型上下文一致。
- 真实 token 成本仍以 provider 的 `turn usage` 为准；预算检查用字符估算是保守近似。

## 指令与缓存

- 每次 run 从 cwd 向上发现 `AGENTS.md`，近者优先，追加在稳定 base system prompt 之后；修改后下一次用户 turn 生效。
- 加载来源与 digest 写入 `instructions` 事件；`session show` 可查。
- 缓存命中率来自 provider 返回的 usage，按 turn 显示 `cache N%`，`done` 行显示总命中率。
- 不写单独 metrics 文件；运行时 Runs 视图留给后续 runtime viewer（ADR-0009）。

## 取消与事件

TUI 调用与 headless 相同的 `run_with_history`，只消费 `AgentEvent`，不 import loop 内部。

- 取消会 drop provider stream 和工具 future；bash 的进程组由 `ProcessGroupGuard` 清理。
- 取消后 session 追加 `session_end`，已完成的事件保留。
- 每次运行仍有 `policy` / `approval` / `instructions` / `tool_result` 审计。
- 同一进程内复用 session 与 history；每次 turn 后从日志重新投影，避免 UI 自己维护第二份对话状态。
- 工具输出折叠只影响呈现；`tool_result` 事件和 session 日志保留完整 output。

## 边界

- 会话本身仍是线性输出；审批 diff 是 alternate-screen overlay，没有常驻滚动区、面板和鼠标。
- 审批时 Ctrl-C 按拒绝处理，不是取消整轮。
- 输出折叠只对工具结果生效；模型流式文本直接输出。
- `:last` 只记录当前进程的最近一次工具结果，不跨重启；需要历史时读 session JSONL。
- 当前 `write` / `edit` 每次只改一个文件；pager 已支持多文件跳转，等后续 patch 工具接入。
- 大 diff 会在进入 pager 时完整布局；极端大文件后续需要预算或分页。
- 大输出仍受 bash 20KB 尾部预算约束。
- 上下文预算是估算值，不替代模型真实 tokenizer。
- 指令在一次 run 内保持稳定；长 run 中途修改 `AGENTS.md` 需要下一次用户 turn 才生效。
- 命令补全只覆盖 slash 命令名，不做文件路径补全；避免把输入行变成第二套 shell。
- `subagent` 子运行只读且并行，完整 transcript 在独立子 session 日志里；TUI 只显示摘要和路径。
