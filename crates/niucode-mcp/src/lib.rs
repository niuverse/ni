//! Minimal MCP stdio client.
//!
//! This crate speaks the subset in ADR-0033 and ADR-0034: `initialize`,
//! `tools/list`, and `tools/call` over newline-delimited JSON on a child
//! process's stdio. Read-only servers mount by default. A server with
//! `read_only = false` needs an explicit allowlist and an explicit allow flag;
//! the caller is responsible for per-call approval (ADR-0034).

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use niucode_provider::ToolSpec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::timeout;

const PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct McpServerSpec {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub read_only: bool,
    /// Tools that may mutate. Every mutating tool must be named; an unlisted
    /// tool from a write-enabled server stays read-only.
    #[serde(default)]
    pub write_tools: Vec<String>,
}

impl McpServerSpec {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            bail!("MCP server name is empty");
        }
        if self.command.trim().is_empty() {
            bail!("MCP server {} has an empty command", self.name);
        }
        for tool in &self.write_tools {
            if tool.trim().is_empty() {
                bail!("MCP server {} has an empty write_tools entry", self.name);
            }
        }
        if self.read_only && !self.write_tools.is_empty() {
            bail!(
                "MCP server {} is read_only but names write_tools",
                self.name
            );
        }
        Ok(())
    }

    /// Validate the write policy. Read-only servers ignore the flag; a
    /// write-enabled server needs both the flag and a non-empty allowlist.
    pub fn validate_policy(&self, allow_write: bool) -> Result<()> {
        self.validate()?;
        if self.read_only {
            return Ok(());
        }
        if !allow_write {
            bail!(
                "MCP server {} is not read_only; pass --allow-mcp-write to mount it",
                self.name
            );
        }
        if self.write_tools.is_empty() {
            bail!(
                "MCP server {} is not read_only but has no write_tools allowlist",
                self.name
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
pub struct McpConfig {
    #[serde(default)]
    pub server: Vec<McpServerSpec>,
}

impl McpConfig {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.is_file() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path)
            .with_context(|| format!("read MCP config {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parse MCP config {}", path.display()))
    }

    /// Read-only validation used by the default mount path.
    pub fn validated(&self) -> Result<Vec<McpServerSpec>> {
        for server in &self.server {
            server.validate()?;
            if !server.read_only {
                bail!(
                    "MCP server {} is not read_only; v1 only starts read-only servers without --allow-mcp-write",
                    server.name
                );
            }
        }
        Ok(self.server.clone())
    }

    /// Validation with an explicit write flag. A write-enabled server still
    /// needs a non-empty `write_tools` allowlist.
    pub fn validated_with(&self, allow_write: bool) -> Result<Vec<McpServerSpec>> {
        for server in &self.server {
            server.validate_policy(allow_write)?;
        }
        Ok(self.server.clone())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct McpTool {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpCall {
    pub ok: bool,
    pub output: String,
}

/// Namespaced name exposed to the agent. Server/tool characters outside the
/// conservative set are replaced so the result is always a valid tool name.
pub fn exposed_name(server: &str, tool: &str) -> String {
    format!("mcp__{}__{}", sanitize(server), sanitize(tool))
}

fn sanitize(value: &str) -> String {
    let mut out = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
            out.push(character);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "unnamed".to_string()
    } else {
        out
    }
}

pub struct McpClient {
    server: String,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpClient {
    pub async fn connect(spec: &McpServerSpec, limit: Duration) -> Result<Self> {
        Self::connect_with(spec, limit, false).await
    }

    async fn connect_with(
        spec: &McpServerSpec,
        limit: Duration,
        allow_write: bool,
    ) -> Result<Self> {
        spec.validate_policy(allow_write)?;
        let mut command = Command::new(&spec.command);
        command
            .args(&spec.args)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (key, value) in &spec.env {
            command.env(key, value);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("spawn MCP server {}", spec.name))?;
        let stdin = child.stdin.take().context("MCP server has no stdin pipe")?;
        let stdout = BufReader::new(
            child
                .stdout
                .take()
                .context("MCP server has no stdout pipe")?,
        );
        let mut client = Self {
            server: spec.name.clone(),
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        let initialized = client
            .request(
                "initialize",
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {"name": "niucode", "version": env!("CARGO_PKG_VERSION")}
                }),
                limit,
            )
            .await?;
        if initialized.get("protocolVersion").is_none() {
            bail!("MCP server {} returned no protocolVersion", spec.name);
        }
        client
            .notify("notifications/initialized", json!({}), limit)
            .await?;
        Ok(client)
    }

    pub fn server_name(&self) -> &str {
        &self.server
    }

    pub async fn list_tools(&mut self, limit: Duration) -> Result<Vec<McpTool>> {
        let result = self.request("tools/list", json!({}), limit).await?;
        let tools = result
            .get("tools")
            .cloned()
            .context("MCP tools/list returned no tools")?;
        serde_json::from_value(tools).context("parse MCP tools/list response")
    }

    pub async fn call_tool(
        &mut self,
        name: &str,
        arguments: Value,
        limit: Duration,
    ) -> Result<McpCall> {
        let result = self
            .request(
                "tools/call",
                json!({"name": name, "arguments": arguments}),
                limit,
            )
            .await?;
        let ok = !result
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut output = String::new();
        if let Some(content) = result.get("content").and_then(Value::as_array) {
            for item in content {
                if item.get("type").and_then(Value::as_str) != Some("text") {
                    continue;
                }
                let text = item.get("text").and_then(Value::as_str).unwrap_or("");
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str(text);
            }
        }
        if output.is_empty() {
            output = result.to_string();
        }
        Ok(McpCall { ok, output })
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        let _ = self.child.start_kill();
        let _ = self.child.wait().await;
        Ok(())
    }

    async fn notify(&mut self, method: &str, params: Value, limit: Duration) -> Result<()> {
        let message = json!({"jsonrpc": "2.0", "method": method, "params": params});
        self.send(&message, limit).await
    }

    async fn request(&mut self, method: &str, params: Value, limit: Duration) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.send(&message, limit).await?;
        loop {
            let response = self.read_message(limit).await?;
            if response.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = response.get("error") {
                bail!("MCP {method} failed: {error}");
            }
            return Ok(response.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    async fn send(&mut self, message: &Value, limit: Duration) -> Result<()> {
        let mut line = serde_json::to_string(message)?;
        line.push('\n');
        match timeout(limit, self.stdin.write_all(line.as_bytes())).await {
            Ok(result) => result.context("write MCP request")?,
            Err(_) => {
                let _ = self.child.start_kill();
                bail!("MCP write timed out");
            }
        }
        match timeout(limit, self.stdin.flush()).await {
            Ok(result) => result.context("flush MCP request")?,
            Err(_) => {
                let _ = self.child.start_kill();
                bail!("MCP flush timed out");
            }
        }
        Ok(())
    }

    async fn read_message(&mut self, limit: Duration) -> Result<Value> {
        let mut line = String::new();
        let read = self.stdout.read_line(&mut line);
        let bytes = match timeout(limit, read).await {
            Ok(result) => result.context("read MCP response")?,
            Err(_) => {
                let _ = self.child.start_kill();
                bail!("MCP response timed out");
            }
        };
        if bytes == 0 {
            bail!("MCP server closed stdout");
        }
        serde_json::from_str(line.trim()).context("parse MCP response")
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpMount {
    pub server: String,
    pub config_hash: String,
    pub tools: Vec<String>,
    pub write_tools: Vec<String>,
}

fn config_hash(spec: &McpServerSpec) -> String {
    // Stable local audit value. Env keys only: values may be secrets.
    let mut hash: u64 = 0xcbf29ce484222325;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    };
    feed(spec.name.as_bytes());
    feed(spec.command.as_bytes());
    for arg in &spec.args {
        feed(arg.as_bytes());
    }
    feed(&[u8::from(spec.read_only)]);
    for tool in &spec.write_tools {
        feed(tool.as_bytes());
    }
    for key in spec.env.keys() {
        feed(key.as_bytes());
    }
    format!("{hash:016x}")
}

struct HostTool {
    server: usize,
    tool: String,
    spec: ToolSpec,
    mutating: bool,
}

/// A set of mounted read-only MCP servers. Calls are serialized per server and
/// may run in parallel across servers; the host owns the child processes.
pub struct McpHost {
    clients: Vec<Mutex<McpClient>>,
    tools: Vec<HostTool>,
    servers: Vec<String>,
    hashes: Vec<String>,
}

impl McpHost {
    pub async fn mount(servers: &[McpServerSpec], limit: Duration) -> Result<Self> {
        Self::mount_with(servers, limit, false).await
    }

    /// Mount with an explicit write flag. The caller must also approve every
    /// mutating call; this method only enforces the manifest allowlist.
    pub async fn mount_with(
        servers: &[McpServerSpec],
        limit: Duration,
        allow_write: bool,
    ) -> Result<Self> {
        let mut clients: Vec<Mutex<McpClient>> = Vec::new();
        let mut tools = Vec::new();
        let mut server_names: Vec<String> = Vec::new();
        let mut hashes: Vec<String> = Vec::new();
        let mut names = HashSet::new();
        for server in servers {
            server.validate_policy(allow_write)?;
            let mut client = McpClient::connect_with(server, limit, allow_write).await?;
            let listed = client.list_tools(limit).await?;
            let listed_names = listed
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<HashSet<_>>();
            for wanted in &server.write_tools {
                if !listed_names.contains(wanted.as_str()) {
                    bail!(
                        "MCP server {} lists write tool {} but does not expose it",
                        server.name,
                        wanted
                    );
                }
            }
            let server_index = clients.len();
            for tool in listed {
                let mutating = !server.read_only && server.write_tools.contains(&tool.name);
                let name = exposed_name(&server.name, &tool.name);
                if !names.insert(name.clone()) {
                    bail!("MCP tool name collision: {name}");
                }
                let description = if tool.description.is_empty() {
                    format!("MCP tool {} from {}", tool.name, server.name)
                } else {
                    tool.description
                };
                let schema = if tool.input_schema.is_null() {
                    json!({"type": "object"})
                } else {
                    tool.input_schema
                };
                tools.push(HostTool {
                    server: server_index,
                    tool: tool.name,
                    spec: ToolSpec::function(name, description, schema),
                    mutating,
                });
            }
            clients.push(Mutex::new(client));
            server_names.push(server.name.clone());
            hashes.push(config_hash(server));
        }
        Ok(Self {
            clients,
            tools,
            servers: server_names,
            hashes,
        })
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        self.tools.iter().map(|tool| tool.spec.clone()).collect()
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.tools
            .iter()
            .any(|tool| tool.spec.function.name == name)
    }

    pub fn server_for(&self, name: &str) -> Option<&str> {
        self.tools
            .iter()
            .find(|tool| tool.spec.function.name == name)
            .and_then(|tool| self.servers.get(tool.server))
            .map(String::as_str)
    }

    pub fn mounts(&self) -> Vec<McpMount> {
        let mut mounts: Vec<McpMount> = self
            .servers
            .iter()
            .enumerate()
            .map(|(index, server)| McpMount {
                server: server.clone(),
                config_hash: self.hashes.get(index).cloned().unwrap_or_default(),
                tools: Vec::new(),
                write_tools: Vec::new(),
            })
            .collect();
        for tool in &self.tools {
            if let Some(mount) = mounts.get_mut(tool.server) {
                mount.tools.push(tool.spec.function.name.clone());
                if tool.mutating {
                    mount.write_tools.push(tool.spec.function.name.clone());
                }
            }
        }
        mounts
    }

    /// Whether the exposed tool was named in a write-enabled server's
    /// allowlist. The agent must approve these calls before dispatch.
    pub fn is_mutating(&self, name: &str) -> bool {
        self.tools
            .iter()
            .any(|tool| tool.spec.function.name == name && tool.mutating)
    }

    pub fn mutating_tools(&self) -> Vec<String> {
        self.tools
            .iter()
            .filter(|tool| tool.mutating)
            .map(|tool| tool.spec.function.name.clone())
            .collect()
    }

    pub async fn call(&self, name: &str, arguments: Value, limit: Duration) -> Result<McpCall> {
        let tool = self
            .tools
            .iter()
            .find(|tool| tool.spec.function.name == name)
            .with_context(|| format!("unknown MCP tool {name}"))?;
        let client = self
            .clients
            .get(tool.server)
            .with_context(|| format!("MCP server for {name} is not mounted"))?;
        let mut client = client.lock().await;
        client.call_tool(&tool.tool, arguments, limit).await
    }

    pub async fn shutdown(&self) -> Result<()> {
        for client in &self.clients {
            let mut client = client.lock().await;
            client.shutdown().await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_namespaced_and_sanitized() {
        assert_eq!(exposed_name("notes", "search"), "mcp__notes__search");
        assert_eq!(exposed_name("my notes", "a.b"), "mcp__my_notes__a_b");
        assert_eq!(exposed_name("", ""), "mcp__unnamed__unnamed");
    }

    #[test]
    fn config_rejects_writable_servers() {
        let spec = McpServerSpec {
            name: "notes".into(),
            command: "python3".into(),
            args: vec![],
            env: BTreeMap::new(),
            read_only: false,
            write_tools: Vec::new(),
        };
        assert!(spec.validate().is_ok());
        assert!(spec
            .validate_policy(false)
            .unwrap_err()
            .to_string()
            .contains("allow-mcp-write"));
        let config = McpConfig {
            server: vec![spec.clone()],
        };
        assert!(config.validated().is_err());
        assert!(config
            .validated_with(true)
            .unwrap_err()
            .to_string()
            .contains("write_tools"));
        let mut allowed = spec;
        allowed.write_tools = vec!["write_note".into()];
        let config = McpConfig {
            server: vec![allowed],
        };
        assert!(config.validated_with(true).is_ok());
        assert!(config.validated().is_err());
    }

    #[test]
    fn missing_config_is_empty() {
        let directory = tempfile::tempdir().unwrap();
        let config = McpConfig::load(&directory.path().join("missing.toml")).unwrap();
        assert!(config.server.is_empty());
    }

    #[test]
    fn parses_the_manifest_shape() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp.toml");
        fs::write(
            &path,
            r#"
[[server]]
name = "notes"
command = "python3"
args = ["server.py"]
read_only = true

[server.env]
NOTES_DIR = "/tmp/notes"
"#,
        )
        .unwrap();
        let config = McpConfig::load(&path).unwrap();
        let servers = config.validated().unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].env["NOTES_DIR"], "/tmp/notes");
    }
}
