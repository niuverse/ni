//! Sidecar extension host.
//!
//! A plugin is a local process speaking line-delimited JSON-RPC over stdio.
//! Tools are discovered during `initialize`; calls are serialized per plugin
//! for now (the manifest does not yet declare a concurrency model).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use niucode_provider::ToolSpec;
use niucode_tools::ToolResult;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::timeout;

const PROTOCOL_VERSION: u64 = 1;
const DEFAULT_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone)]
pub struct MountInfo {
    pub name: String,
    pub version: String,
    pub tools: Vec<String>,
    pub manifest: PathBuf,
    /// Read-only plugins may be exposed to subagent Explore/Worker profiles.
    pub read_only: bool,
    pub capabilities: Vec<String>,
    pub digest: String,
    pub workers: usize,
}

pub struct MountReport {
    pub host: ExtensionHost,
    pub errors: Vec<String>,
}

/// Discover `.niucode/extensions/*.toml` under the workspace, spawn each
/// sidecar, and register its tools. Reserved tool names (usually built-ins)
/// are rejected rather than shadowed.
pub async fn discover(workspace: &Path, reserved: &[String]) -> MountReport {
    let directory = workspace.join(".niucode/extensions");
    discover_in(&directory, workspace, reserved).await
}

/// Discover manifests from an explicit directory while resolving relative
/// command paths against `workspace`. The CLI uses the global per-project
/// extension store; tests can point at a temp directory directly.
pub async fn discover_in(directory: &Path, workspace: &Path, reserved: &[String]) -> MountReport {
    let mut host = ExtensionHost::default();
    let mut errors = Vec::new();
    let mut reserved = reserved.iter().cloned().collect::<HashSet<_>>();

    if !directory.is_dir() {
        return MountReport { host, errors };
    }

    let mut manifests = match std::fs::read_dir(directory) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "toml")
            })
            .collect::<Vec<_>>(),
        Err(error) => {
            errors.push(format!("read {}: {error}", directory.display()));
            return MountReport { host, errors };
        }
    };
    manifests.sort();

    for path in manifests {
        match mount_one(&path, workspace).await {
            Ok((pool, info)) => {
                let mut collision = None;
                for tool in &info.tools {
                    if !reserved.insert(tool.clone()) {
                        collision = Some(tool.clone());
                        break;
                    }
                }
                if let Some(tool) = collision {
                    errors.push(format!(
                        "{}: tool {tool:?} collides with a built-in or another extension",
                        info.name
                    ));
                    continue;
                }
                host.register(pool, info);
            }
            Err(error) => errors.push(format!("{}: {error:#}", path.display())),
        }
    }

    MountReport { host, errors }
}

struct Capabilities {
    fs_write: bool,
    network: bool,
    subprocess: bool,
}

impl Capabilities {
    fn parse(values: &[String]) -> Result<Self> {
        let mut capabilities = Self {
            fs_write: false,
            network: false,
            subprocess: false,
        };
        for value in values {
            match value.as_str() {
                "fs_read" => {}
                "fs_write" => capabilities.fs_write = true,
                "network" => capabilities.network = true,
                "subprocess" => capabilities.subprocess = true,
                other => bail!("unknown extension capability {other:?}"),
            }
        }
        Ok(capabilities)
    }
}

async fn mount_one(path: &Path, workspace: &Path) -> Result<(Pool, MountInfo)> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let manifest: Manifest =
        toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    validate_name(&manifest.name)?;
    if manifest.command.trim().is_empty() {
        bail!("manifest command is empty");
    }
    if manifest.protocol != PROTOCOL_VERSION {
        bail!(
            "unsupported extension protocol {} (host supports {PROTOCOL_VERSION})",
            manifest.protocol
        );
    }
    let capabilities = Capabilities::parse(&manifest.capabilities)?;
    let workers = manifest.workers.clamp(1, 8);
    let read_only = manifest.read_only && !capabilities.fs_write;
    let digest = digest(&format!(
        "{}:{}:{}:{}:{}",
        manifest.name,
        manifest.command,
        manifest.args.join("\u{1f}"),
        manifest.capabilities.join(","),
        manifest.workers
    ));

    let mut plugins = Vec::new();
    let mut first: Option<(String, Vec<ToolSpec>, Vec<String>)> = None;
    for worker in 0..workers {
        let mut plugin = spawn_plugin(&manifest, workspace, &capabilities, worker)?;
        let initialized = initialize_plugin(&mut plugin).await?;
        if let Some((version, specs, _names)) = &first {
            if specs.len() != initialized.1.len()
                || specs
                    .iter()
                    .zip(&initialized.1)
                    .any(|(left, right)| left.function.name != right.function.name)
            {
                bail!(
                    "extension {} declared inconsistent tools across workers",
                    manifest.name
                );
            }
            if version != &initialized.0 {
                bail!("extension {} reported inconsistent versions", manifest.name);
            }
        } else {
            first = Some(initialized.clone());
        }
        plugins.push(plugin);
    }
    let (version, specs, names) = first.context("extension worker pool is empty")?;
    Ok((
        Pool::new(plugins, specs.clone()),
        MountInfo {
            name: manifest.name,
            version,
            tools: names,
            manifest: path.to_path_buf(),
            read_only,
            capabilities: manifest.capabilities,
            digest,
            workers,
        },
    ))
}

fn spawn_plugin(
    manifest: &Manifest,
    workspace: &Path,
    capabilities: &Capabilities,
    worker: usize,
) -> Result<Plugin> {
    let mut command = Command::new("bwrap");
    command
        .arg("--die-with-parent")
        .arg("--new-session")
        .arg("--unshare-pid")
        .arg("--unshare-ipc")
        .arg("--unshare-uts");
    if !capabilities.network {
        command.arg("--unshare-net");
    }
    for directory in ["/usr", "/bin", "/lib", "/lib64", "/etc"] {
        command.arg("--ro-bind-try").arg(directory).arg(directory);
    }
    command.arg("--tmpfs").arg("/tmp");
    command.arg("--dev").arg("/dev");
    command.arg("--proc").arg("/proc");
    if capabilities.fs_write {
        command.arg("--bind").arg(workspace).arg(workspace);
    } else {
        command.arg("--ro-bind").arg(workspace).arg(workspace);
    }
    command
        .arg("--chdir")
        .arg(workspace)
        .arg("--clearenv")
        .arg("--setenv")
        .arg("PATH")
        .arg(std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into()))
        .arg("--setenv")
        .arg("HOME")
        .arg(workspace)
        .arg("--setenv")
        .arg("NIUCODE_EXTENSION")
        .arg("1")
        .arg("--setenv")
        .arg("PYTHONDONTWRITEBYTECODE")
        .arg("1")
        .arg("--setenv")
        .arg("NIUCODE_EXTENSION_WORKER")
        .arg(worker.to_string())
        .arg("--")
        .arg(&manifest.command)
        .args(&manifest.args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut child = command
        .spawn()
        .with_context(|| format!("spawn extension {} under bwrap", manifest.name))?;
    let stdin = child.stdin.take().context("extension stdin unavailable")?;
    let stdout = child
        .stdout
        .take()
        .context("extension stdout unavailable")?;
    Ok(Plugin {
        name: manifest.name.clone(),
        child,
        stdin: BufWriter::new(stdin),
        stdout: BufReader::new(stdout),
        next_id: 1,
        timeout: Duration::from_secs(manifest.timeout_secs.max(1)),
        tools: Vec::new(),
    })
}

async fn initialize_plugin(plugin: &mut Plugin) -> Result<(String, Vec<ToolSpec>, Vec<String>)> {
    let init = plugin
        .request(
            "initialize",
            json!({
                "protocol": PROTOCOL_VERSION,
                "client": "niucode",
            }),
        )
        .await
        .with_context(|| format!("initialize extension {}", plugin.name))?;
    let version = init
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("0.0.0")
        .to_string();
    let tools = init
        .get("tools")
        .and_then(Value::as_array)
        .context("initialize result has no tools array")?;
    let mut specs = Vec::new();
    let mut names = Vec::new();
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .context("extension tool missing name")?;
        validate_tool_name(name)?;
        let description = tool
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("External extension tool.");
        let schema = tool
            .get("input_schema")
            .cloned()
            .unwrap_or_else(|| json!({"type": "object", "properties": {}}));
        names.push(name.to_string());
        specs.push(ToolSpec::function(name, description, schema));
    }
    if specs.is_empty() {
        bail!("extension {} registered no tools", plugin.name);
    }
    plugin.tools = specs.clone();
    Ok((version, specs, names))
}

struct Plugin {
    name: String,
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    timeout: Duration,
    tools: Vec<ToolSpec>,
}

impl Plugin {
    async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let line = serde_json::to_string(&message).context("encode extension request")?;
        self.stdin
            .write_all(line.as_bytes())
            .await
            .context("write extension request")?;
        self.stdin
            .write_all(b"\n")
            .await
            .context("write extension newline")?;
        self.stdin
            .flush()
            .await
            .context("flush extension request")?;

        let mut response = String::new();
        let read = timeout(self.timeout, self.stdout.read_line(&mut response))
            .await
            .with_context(|| format!("extension {} timed out", self.name))??;
        if read == 0 {
            bail!("extension {} closed stdout", self.name);
        }
        let value: Value =
            serde_json::from_str(response.trim()).context("decode extension response")?;
        if let Some(error) = value.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown extension error");
            bail!("extension {}: {message}", self.name);
        }
        let response_id = value.get("id").and_then(Value::as_u64);
        if response_id != Some(id) {
            bail!(
                "extension {} returned response id {:?}, expected {id}",
                self.name,
                response_id
            );
        }
        Ok(value.get("result").cloned().unwrap_or(Value::Null))
    }

    async fn call_tool(&mut self, name: &str, input: &Value) -> Result<ToolResult> {
        let result = self
            .request("tools/call", json!({"name": name, "arguments": input}))
            .await?;
        let ok = result.get("ok").and_then(Value::as_bool).unwrap_or(false);
        let output = result
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or("extension returned no output")
            .to_string();
        Ok(ToolResult { ok, output })
    }
}

struct Pool {
    workers: Vec<Arc<Mutex<Plugin>>>,
    next: AtomicUsize,
    tools: Vec<ToolSpec>,
}

impl Pool {
    fn new(plugins: Vec<Plugin>, tools: Vec<ToolSpec>) -> Self {
        Self {
            workers: plugins
                .into_iter()
                .map(|plugin| Arc::new(Mutex::new(plugin)))
                .collect(),
            next: AtomicUsize::new(0),
            tools,
        }
    }

    async fn execute(&self, name: &str, input: &Value) -> ToolResult {
        if self.workers.is_empty() {
            return ToolResult {
                ok: false,
                output: "extension worker pool is empty".into(),
            };
        }
        let index = self.next.fetch_add(1, Ordering::Relaxed) % self.workers.len();
        let worker = &self.workers[index];
        match worker.lock().await.call_tool(name, input).await {
            Ok(result) => result,
            Err(error) => ToolResult {
                ok: false,
                output: format!("extension error: {error:#}"),
            },
        }
    }

    async fn shutdown(&self) {
        for worker in &self.workers {
            let mut worker = worker.lock().await;
            let _ = worker.request("shutdown", json!({})).await;
            let _ = worker.child.kill().await;
        }
    }
}

#[derive(Default)]
pub struct ExtensionHost {
    plugins: HashMap<String, Arc<Pool>>,
    routes: HashMap<String, String>,
    mounts: Vec<MountInfo>,
    specs: Vec<ToolSpec>,
}

impl ExtensionHost {
    pub fn specs(&self) -> Vec<ToolSpec> {
        self.specs.clone()
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.routes.contains_key(name)
    }

    pub fn mounts(&self) -> &[MountInfo] {
        &self.mounts
    }

    pub fn read_only_tools(&self) -> Vec<String> {
        let mut names = Vec::new();
        for mount in &self.mounts {
            if mount.read_only {
                names.extend(mount.tools.iter().cloned());
            }
        }
        names
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    fn register(&mut self, pool: Pool, info: MountInfo) {
        let name = info.name.clone();
        for tool in &info.tools {
            self.routes.insert(tool.clone(), name.clone());
        }
        self.specs.extend(pool.tools.clone());
        self.mounts.push(info);
        self.plugins.insert(name, Arc::new(pool));
    }

    pub async fn execute(&self, name: &str, input: &Value) -> ToolResult {
        let Some(plugin_name) = self.routes.get(name) else {
            return ToolResult {
                ok: false,
                output: format!("unknown extension tool {name:?}"),
            };
        };
        let Some(pool) = self.plugins.get(plugin_name) else {
            return ToolResult {
                ok: false,
                output: format!("extension {plugin_name:?} is not mounted"),
            };
        };
        pool.execute(name, input).await
    }

    pub async fn shutdown(&self) {
        for pool in self.plugins.values() {
            pool.shutdown().await;
        }
    }
}

fn digest(text: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[derive(Debug, Deserialize)]
struct Manifest {
    name: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default = "default_timeout")]
    timeout_secs: u64,
    #[serde(default = "default_protocol")]
    protocol: u64,
    /// Default true: external tools are treated as read-only for subagent
    /// capability views. Set false only for explicitly trusted mutating tools.
    #[serde(default = "default_read_only")]
    read_only: bool,
    /// Explicit capabilities: fs_read, fs_write, network, subprocess.
    #[serde(default)]
    capabilities: Vec<String>,
    /// Number of sidecar workers for this plugin (1-8).
    #[serde(default = "default_workers")]
    workers: usize,
}

fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

fn default_protocol() -> u64 {
    PROTOCOL_VERSION
}

fn default_read_only() -> bool {
    true
}

fn default_workers() -> usize {
    1
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        bail!("invalid extension name {name:?}");
    }
    Ok(())
}

fn validate_tool_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        bail!("invalid extension tool name {name:?}");
    }
    Ok(())
}
