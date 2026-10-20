+++
id = "auth"
title = "凭证与 niucode auth"
kind = "doc"
status = "living"
order = 31
summary = "API key 的解析顺序、keyring/0600 文件回退、安全约束，以及为什么不做网页账号 cookie。"
+++

## 命令

```sh
niucode auth set deepseek        # 隐藏输入
niucode auth set openai
printf '%s\n' "$KEY" | niucode auth set deepseek --stdin

niucode auth status              # 只显示来源，不显示 key
niucode auth unset deepseek

niucode provider test --preset deepseek
```

`niucode auth set` 会打印 key 的创建页面。key 本身不会回显，也不会写入 session log。

## 解析顺序

`AuthStore::resolve` 固定按以下顺序：

1. **环境变量**：`DEEPSEEK_API_KEY` / `OPENAI_API_KEY`
2. **OS keyring**：service `niucode`，account 是 provider id（`deepseek` / `openai`）
3. **0600 文件**：`$NIUCODE_CONFIG_DIR/credentials.toml`，否则 `$XDG_CONFIG_HOME/niucode/credentials.toml`，否则 `~/.config/niucode/credentials.toml`

环境变量永远优先，适合 CI 和临时覆盖。keyring 可用时写入 keyring；keyring 不可用（无桌面、容器、无 Secret Service）时回退文件。

```sh
# 强制文件回退，避免在 headless 环境触发 keyring
NIUCODE_AUTH_DISABLE_KEYRING=1 niucode auth set deepseek
```

## 文件格式

```toml
[providers.deepseek]
api_key = "sk-..."
```

- 目录不存在时创建为 `0700`；文件写入临时文件后 `rename`，权限 `0600`。
- `niucode auth status` 在文件权限过宽时显示 `insecure` 并警告。
- 解析失败只返回“credentials file is not valid TOML”，不回显文件内容，避免错误信息泄露 key。
- 不支持把 key 写进仓库配置、session JSONL 或日志。

## Linux 依赖

Linux 的 keyring 走 Secret Service（DBus）。构建需要 `libdbus-1-dev`；运行需要桌面 keyring 服务。两者都没有时，用 `NIUCODE_AUTH_DISABLE_KEYRING=1` 或让程序自动回退到文件。

macOS / Windows 使用系统 keychain / Credential Manager。

## 为什么不做网页账号登录

- DeepSeek 没有面向第三方客户端的公开账号 OAuth；官方接入是 API key。
- ChatGPT 账号登录有官方的 [Sign in with ChatGPT](https://developers.openai.com/siwc/quickstart)，但需要 OAuth + Responses API，且面向本地开源/个人项目；它是独立的 provider/auth 项目，不是“复用网页 cookie”。
- 抓 cookie / 模拟网页请求会带来 ToS、封号、WAF、2FA 和密钥泄露风险，不作为 harness 的能力。

详见 [Provider 与 DeepSeek 接入](#/docs/provider) 和 [Roadmap](#/docs/roadmap)。

## 安全边界

- `SecretKey` 的 `Debug` 输出是 `SecretKey(***)`；命令行只打印来源。
- 每次 `niucode auth status` 会查询 keyring；在锁屏/未解锁的桌面上可能触发系统解锁提示。
- `niucode auth unset` 同时删除 keyring 和文件；不会尝试猜测其他存储位置。
- 目前没有做内存 zeroize；key 在进程生命周期内存在。后续如果需要更强保证，再引入 `secrecy`/`zeroize` 并重新评估。
