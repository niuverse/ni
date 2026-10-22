+++
id = "tui"
title = "Inline TUI、Diff Pager 与审批"
kind = "doc"
status = "living"
order = 33
summary = "原生 scrollback、紧凑任务收尾、语义高亮与活动区动画，完整证据通过 Ctrl-O 查看。"
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

## 交互与编辑

TTY 中 linear 与 inline 共用同一编辑器；主 transcript 留在原生 scrollback，审批和详情临时使用 alternate screen。

| 按键 | 行为 |
|---|---|
| Enter | 空闲时提交任务；运行中补充当前任务，在安全的下一请求边界生效 |
| Alt-Enter | 运行中提交独立排队任务，按 FIFO 执行 |
| Ctrl-J / Shift-Enter | 插入换行；Shift-Enter 需要终端能区分该组合 |
| Ctrl-C | 运行中取消，包括审批等待；保留已完成修改和草稿；空闲时清空输入 |
| Ctrl-D | 空输入时退出；非空时删除光标后的字素 |
| Ctrl-O / `/last` | 打开已记录历史、reasoning、工具输出及任务累计 diff |
| Ctrl-Z / Ctrl-Y | 撤销 / 重做；一次 bracketed paste 是一个撤销单元 |
| ↑ / ↓ | 菜单选择；无菜单时移动显示行，到边界时浏览历史 |
| Ctrl-P / Ctrl-N / Ctrl-R | 上一历史 / 下一历史 / 反向搜索；返回末尾恢复原草稿 |
| Home / End、Ctrl-A / Ctrl-E | 当前逻辑行首 / 行尾 |
| Ctrl-Left/Right、Alt-B/F | 按词移动 |
| Ctrl-W / Ctrl-U / Ctrl-K | 删前一个词 / 删到行首 / 删到行尾 |
| Ctrl-L | 重绘并清除当前屏幕，保留输入和原生 scrollback |
| Tab / Esc | 接受补全 / 关闭候选 |

- 多行粘贴保留换行，CRLF 归一为 LF；粘贴不提交、不执行其中的 slash 命令。
- 中文、组合字符和 ZWJ emoji 按字素编辑，换行与光标共用显示列布局。输入上限 1 MiB，撤销和历史分别有 8 MiB / 4 MiB 预算。
- 流式预览、inline transcript、详情和 diff 共用字素换行；满行输出不增加空白行，输入保留行尾光标位置。tab 按逻辑行四列制表位展开；超过屏幕宽度的单个字素显示为 `�`。
- 输出布局每个逻辑行最多展示 1 MiB 源文本，超出时保留字素完整的前缀并提示 `[display truncated]`，后续行继续显示。linear transcript 保留终端原生软换行；原始会话日志不因显示裁剪而修改。
- `/` 命令、`/model` / `/permissions` 参数、`$skill`、`@path` 在光标 token 处补全。文件发现遵循嵌套 ignore，后台扫描可取消，最多 10000 个文件 / 2 秒；超限明确提示不完整。
- 输入区最多 6 行，候选最多 4 行；小窗口缩减活动区。模型未结束的当前文本行显示在活动区，完整行写入 transcript。
- 输入焦点使用稳定竖线光标；菜单焦点隐藏输入光标，以高亮候选和操作提示表示当前位置。继续键入返回编辑；离开 TUI 恢复终端默认光标形状。
- 鼠标仅在有绝对坐标的 inline 菜单内捕获。linear 不猜测鼠标位置；关闭菜单恢复终端原生选择复制。
- reasoning 默认收起；Ctrl-O 查看已记录内容。流中断前尚未形成 `assistant_message` 的部分无法完整恢复。
- 运行中显示 pending / applied / queued；成功落日志才确认接收。消息去重、队列容量 32、单条运行中输入 64 KiB；拒绝会说明原因并保留原文。
- 已接受队列从日志恢复；启动或 `/resume` 本身不自动执行，需要再次提交任务。未提交草稿只存在内存。
- 运行中本地 `/` 命令不发给模型；先取消或等待任务结束，再切换 model/permissions/session。
- `exit` / `quit` / `/exit` / `:q` 在空闲时退出；取消不通过 `process::exit` 绕过终端恢复。

Ghostty 默认保留 linear；`NIUCODE_TUI_INLINE=1` 显式试用 inline，`NIUCODE_TUI_LINEAR=1` 显式选择 linear。光标查询失败会显示 fallback 原因。真实平台验收见 [终端验收记录](terminal-acceptance.md)。

非 TTY 保留逐行输入和原有 exec JSON 协议；不启用行编辑、鼠标或运行中按键交互。颜色遵循 `NO_COLOR` / `CLICOLOR=0` / `TERM=dumb`；终端控制序列仍用于 TTY 布局。

## 历史详情与任务审阅

`Ctrl-O` / `/last` 从 session JSONL 建索引，`[` / `]` 切换记录，↑↓、j/k、PageUp/PageDown 滚动，Esc/q 返回。关闭后恢复草稿和光标；运行中仍可取消。后台 transcript 最多暂存 256 KiB，超过预算提示去日志详情查看。

成功工具调用在活动区合并计数，错误显示一行原因；完整命令和输出从 Ctrl-O 查看。每任务只收尾一次，系统收尾最多 3 行，与文件数量无关；正常回答和代码不受这个摘要预算裁剪。Ctrl-O 查看累计 diff。基线是任务开始时磁盘内容，包含已有 dirty/untracked 文件，不改用户 index/stash。无法归属的变化明确写 origin unknown；二进制、缺失、大文件和不完整扫描不伪造文本 diff。

`exit 0` 只表示该命令成功退出。普通无工具对话不提示缺少检查；有实际修改但没运行检查时显示 no checks，检查后工作区变化则可能过期。TaskFinished complete 表示 agent 运行结束，不表示全部验收通过。命令输出仍受工具捕获预算限制，详情展示的是实际记录内容。

## 高亮、动效和证据覆盖

启动只显示项目/模型与权限提示两行，完整运行配置通过 `/status` 查看。
主题高亮标题、行内代码、代码围栏中的关键字/数字、diff 增删与任务结果；先清洗外部文本，再由终端层生成样式。
两种呈现模式使用相同的样式规则。状态转轮每 125ms 更新一帧，只画在活动区；空闲时不动画，未变化帧不重绘。
`NO_COLOR` / `CLICOLOR=0` / `TERM=dumb` 同时禁用装饰性颜色和转轮；状态文字、快捷键和运行结果保留。

快照超预算代表覆盖未知，不代表文件被修改。详情列出覆盖原因，主屏不逐文件打印。
变更计数只包含已证实净变化；存在写入记录但净变化无法确定时显示 recorded writes，不把未知说成没有修改。
相同内容按已校验 digest 复用日志中更早的正文；恢复时不读取当前磁盘充当历史内容，丢失前缀时明确无法恢复 diff。
审批打断历史阅读后，批准/拒绝会恢复原记录与滚动位置；只有普通 a/s/d 按键作审批决定，Ctrl-A 不批准，Ctrl-Q 不隐藏仍在等待的审批。

具体测试范围和性能记录见 [TUI 升级验证](tui-upgrade-validation.md)。真实终端和 macOS 验收仍单独跟踪。

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
| `/model [name]` | 输入 `/model` 后 Enter 打开模型下拉；↑↓ 后 Enter 或鼠标点击直接应用，返回输入区。 |
| `/compact` | 立即生成确定性 handoff 并写入 compaction 事件；下一次 turn 使用压缩后的投影。 |
| `/permissions [profile]` | Enter 打开 profile 下拉；↑↓ 后 Enter 或鼠标点击直接切换 ask/review/full-access/ci。 |
| `/plan` | 进入只读 plan mode；`/plan off` / `/plan approve` 退出；`/plan status` 查看。 |
| `/goal <condition>` | 设置目标并立即启动 goal run（最多 6 轮）；`/goal` 查看；`/goal run` 重新启动；`/goal stop` 停止；`/goal clear` 清除。 |
| `/fork [event-id]` | 复制当前 session 的前缀到新 session 并切换过去；默认 fork 到最后一个事件。 |
| `/status` | 显示 model、cwd、sandbox、context budget、session、事件数、turn / compaction / instruction / subagent / extension 计数和累计 cache。 |
| `/session` | 显示当前 session 路径、事件数和 turn 数。 |
| `/last` | TTY 打开历史详情；非 TTY 保留最近工具输出。 |
| `/help` | 显示命令表。 |
| `/exit` | 退出。 |

细节：

- `/init` 根据 `Cargo.toml` / `package.json` / `pyproject.toml` / `go.mod` 选择测试、构建、lint 命令；未知项目保留占位注释。它只写 `<cwd>/AGENTS.md`，不跟随已存在的 symlink。
- `/plan` 不改变 profile，只把 tool allowlist 收窄到 `read/grep/glob` 并注入只读计划指令；`/plan approve` 只退出模式，不自动执行。
- `/goal` 不改变权限；active goal 进入 system prompt，并写入 `goal_set` / `goal_cleared` 事件。
- session 默认写入 `~/.niucode/projects/<key>/sessions/`；项目根不创建 `.niucode`。`/clear` 继承当前 model、cwd 和 sandbox 配置，写入新的 `session_start` / `policy` 事件；保留 plan mode 与 goal（重新写入事件），停止 goal run，保留输入历史。
- `/reset` 是更强的重置：新 session + plan off + goal 清空 + pending/输入历史清空 + 重新读取 AGENTS.md；extension host 仍按 session 生命周期挂载，不重启进程。
- `/resume` 切换 append-only session：重新投影 history 和 UI 状态，停止当前 goal run；无参数进入上下选择 picker（标题取首条 user message），`/resume --all` 跨项目；非 TTY 回退文本列表；cwd/model 不一致会警告。
- `/reload` 只重读 AGENTS.md 指令；扩展 manifest 在 session 启动时挂载，重新加载需要重启 TUI。
- `/model` 是运行时模型选择：canonical `provider/model`；写入 `model_change { provider, model }`，`/resume` 恢复，`/clear` 保留，`/reset` 清除。provider 由 `ProviderFactory` 惰性解析凭据并构造；切换发生在 turn 边界。请求仍然走同一个 endpoint/auth，下一次 turn 使用新 model；切换 turn 可能失去 prompt cache。
- `/model` 和 `/permissions` 输入时预览候选，Enter 打开选择菜单；↑↓ 选择后一次 Enter（或点击）直接应用并返回空输入区。也可以从预览直接用 ↑↓ 选择后 Enter 应用。从 `/` 命令补全选中这两个命令会直接打开参数菜单。打开菜单本身不改配置，Esc 取消；Tab 仅填入参数，仍需 Enter 提交。非 TTY 裸命令继续显示当前值。建议列表来自 `NIUCODE_MODEL_CHOICES`（逗号分隔）或内置的 deepseek/openai 小列表；这是 Pi 式“选择 vs 实际请求模型”的简化版，不含 virtual model/router。
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

审批按键：a 允许、d/Esc/q 拒绝、s 本 session 自动允许；Ctrl-C 取消整个任务。↑↓、j/k、PageUp/PageDown/空格滚动；resize 重排。非 TTY 使用逐行输入。

嵌入方可以通过 `TuiOptions.approval_keys` 覆盖三个按键（例如 niuse 用 y/n/s，避免 d 在 REPL 里表示完整 diff）；pager 页脚、inline view 和逐行提示都会显示实际按键，重复按键会被拒绝。Esc/q/Ctrl-C 始终保留拒绝或取消的安全含义。

文件预览和实际写入绑定同一个 prepared mutation：路径、文件身份、内容摘要、权限与已读行覆盖都重新校验。审批等待不占写锁；批准后获取 W1 再校验。变化后返回结构化 conflict，需要重新读取和预览。临时文件写入成功后原子替换，保留现有权限/可执行位；新文件竞争创建不覆盖。

这是乐观检测；最终校验与 rename 间仍有外部程序竞争窗口，不是 CAS 或事务。任意 shell/扩展写入不能继承文件工具的冲突保证；不自动回滚外部变化。

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

TUI 通过公开的 `run_interactive` Command/Event 通道驱动 agent；headless 使用同一执行引擎的 callback adapter。UI 不调用 loop 内部、不持久化自己的队列。

- 运行时在 provider、工具、审批和写锁等待时处理命令；独立取消通道不受事件队列背压阻塞。
- 取消 drop effect future；shell 与子代理 Git 命令清理进程组。中断的 worktree 保留供检查，不自动删除未保存改动。
- `input_submitted` 在确认前落盘；`input_applied` 进入上下文投影。tool-call batch 配对后才注入补充。
- `task_started` / `task_finished` 与前后快照记录任务范围；缺少结果或最终快照时显示 unknown/incomplete，不自动重放工具。
- 日志追加失败后 writer 停止，避免重复拼接部分事件；不把未写入的输入显示成成功接收。
- 同一 `SessionLog` 在内部串行追加并 flush，没有第二份 durable store。旧事件语义与 `ToolResult.ok` 不变。

## 保证边界

- 自动化和 PTY 不能证明真实终端的 IME、字体、复制、scrollback 或 resize 重排视觉效果；M5 Ghostty gate 保留。
- 快照预算为 2000 文件 / 单文件 512 KiB / 总内容 4 MiB / 扫描约 2 秒；最终清理快照有 3 秒上限。非 Git、超预算或读写期间变化均显示覆盖不足；快照不是原子事务。
- 本地文件系统调用和终端输出仍受 OS/设备阻塞影响；慢终端、网络文件系统与长时间负载需要实机压力验收。
- 快照只记录覆盖范围内的事实；不能靠缓存重建超限未记录内容。
- 输入扫描不跟随目录 symlink；文件工具继续遵守 workspace/read-roots 与既有沙箱策略。
- subagent Explore 保持只读，Worker 默认共享 W1，可显式 worktree；完整子任务记录在独立日志中。
