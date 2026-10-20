+++
id = "readme"
title = "看板维护指南"
kind = "guide"
order = 90
summary = "docs/ 的源文件、构建命令、卡片格式与每次迭代的更新仪式。"
+++

## 这是什么

`docs/index.html` 是项目唯一的阅读面：看板、架构、文档、时间线共享一个三栏壳。它由仓库里的源文件生成，离线可开，不依赖服务端。

```sh
cargo xtask dashboard          # 生成 docs/index.html + model.json + receipt.json
cargo xtask dashboard --check  # CI：生成物过期、引用断开时失败
```

构建是确定性的：相同输入生成相同 HTML。`receipt.json` 记录构建时间、git revision 和输入清单，但不参与过期判断，也不入库。

## 源文件

```text
docs/
  index.html                  # 生成物，提交，file:// 可开
  dashboard/
    project.json              # 项目、里程碑、层、模块、关系、时间线
    template.html             # 单文件壳
    style.css                 # 三栏与四个视图的样式
    app.js                    # 无框架渲染
    model.json                # 生成物：解析后的完整模型
    receipt.json              # 本地审计信息，gitignore，不入库
  board/
    M0-001-workspace.md       # 一张卡一个文件
  adr/
    0001-rust-core.md
    0002-event-log.md
  architecture.md
  README.md
```

**唯一真相**：`project.json` + `board/*.md` + 其余 Markdown。`index.html`、`model.json`、`receipt.json` 永不手改。

## 卡片格式

TODO frontmatter 用 TOML，正文是给详情栏读的 Markdown：

```toml
+++
id = "M0-005"
title = "Provider trait 与流式适配"
status = "ready"          # backlog | ready | doing | review | done | blocked
milestone = "M0"
module = "providers"      # 可省略；必须是 project.json 里的模块 id
size = "L"                # S | M | L
priority = "P0"           # 可省略；P0 | P1 | P2
updated = "2026-10-20"
completed = "2026-10-20"  # done 必填
depends_on = ["M0-002"]
docs = ["architecture"]   # 可选；必须是文档 id
blocked_reason = ""       # blocked 必填
+++
```

正文建议四段：`## 范围`、`## 验收`、`## 记录`，需要时加 `## 开放问题`。验收写成可执行的检查，不写"体验良好"。

## 文档格式

所有 `docs/**/*.md`（`board/` 和 `dashboard/` 除外）必须有 frontmatter：

```toml
+++
id = "adr-0003"
title = "ADR-0003：..."
kind = "adr"              # adr | doc | guide
status = "proposed"       # ADR 必填：proposed | accepted | superseded | rejected | draft
order = 30
summary = "一句话，显示在文档卡片上。"
+++
```

文档 id 用于深链：`#/docs/adr-0001`。卡片里的 `docs` 引用按 id 校验。

## 架构模型

`project.json` 里手工维护，不从文件名推断：

- `milestones`：M0–M3，状态 `planned | active | done`，同一时间最多一个 active。
- `layers`：语义分层，不是目录树。
- `modules`：职责、公开契约、拥有的路径、备注。`active` / `done` 至少有一个路径真实存在。
- `relations`：`from` / `to` / `label`，两端必须是模块 id。
- `events`：时间线的手工事件；`refs` 可以指向卡片或文档 id。

## 更新仪式

每次任务结束，按顺序做：

1. 更新对应卡片的 `status`、`updated`、必要的 `completed`。
2. 有架构决策就加一篇 ADR，并在 `project.json` 的 `events` 里加一条。
3. 跑 `cargo fmt --all && cargo test --workspace`。
4. 跑 `cargo xtask dashboard`，把 `docs/index.html` 一起提交。
5. 在 `AGENTS.md` 的 definition of done 里确认这一步没有被跳过。

状态必须反映现实：没有 `doing` 的卡就不要写 `doing`；`done` 必须有完成日期和可验证的验收项。

## 校验规则

构建时会失败的情况：

- schema version 不是 1；id 重复；里程碑 / 模块 / 文档引用不存在。
- `done` 没有 `completed`；`blocked` 没有 `blocked_reason`；日期格式错误。
- `active` / `done` 模块的路径一个都不存在。
- ADR 的 status 不在允许集合。
- 关系端点不是模块。
- `--check` 时生成物与源不一致。

## 视觉约定

- 暖纸色底、墨色文字、一个深绿强调色；状态色只用于状态。
- 状态：Backlog 灰、Ready 蓝、Doing 琥珀、Review 紫、Done 绿、Blocked 红。
- 层色只用于模块卡顶部细线和图例，不铺满背景。
- 数字、ID、路径用等宽字体；正文用系统无衬线。
- 不引 CDN、不引外部字体，保证 `file://` 和离线环境一致。
