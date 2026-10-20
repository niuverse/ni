//! Workspace tools exposed to the model.
//!
//! The first closed loop intentionally has four tools: `read`, `write`, `edit`,
//! and `bash`. All file paths are resolved inside the working directory;
//! `write`/`edit` require the file to have been observed by `read` first.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use globset::GlobBuilder;
use ignore::WalkBuilder;
use ni_provider::ToolSpec;
use regex::Regex;
use serde_json::{json, Value};
use similar::TextDiff;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::time::timeout;

const READ_MAX_LINES: usize = 2_000;
const READ_MAX_BYTES: usize = 50_000;
const BASH_MAX_BYTES: usize = 20_000;
const BASH_DEFAULT_TIMEOUT: u64 = 120;
const BASH_MAX_TIMEOUT: u64 = 600;
const BASH_CAPTURE_BYTES: usize = 8 * 1024 * 1024;
const SEARCH_MAX_RESULTS: usize = 200;

pub struct Toolbox {
    cwd: PathBuf,
    observed: Mutex<HashSet<PathBuf>>,
}

impl Toolbox {
    pub fn new(cwd: impl AsRef<Path>) -> Result<Self> {
        let cwd = cwd.as_ref().canonicalize().with_context(|| {
            format!("canonicalize working directory {}", cwd.as_ref().display())
        })?;
        if !cwd.is_dir() {
            bail!("working directory is not a directory: {}", cwd.display());
        }

        Ok(Self {
            cwd,
            observed: Mutex::new(HashSet::new()),
        })
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn specs() -> Vec<ToolSpec> {
        vec![
            ToolSpec::function(
                "read",
                "Read a UTF-8 text file inside the workspace. Returns line numbers. Read a file before editing or overwriting it.",
                json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "Workspace-relative path"},
                        "offset": {"type": "integer", "description": "1-based first line to return", "minimum": 1},
                        "limit": {"type": "integer", "description": "Maximum lines to return", "minimum": 1, "maximum": READ_MAX_LINES}
                    },
                    "required": ["path"]
                }),
            ),
            ToolSpec::function(
                "write",
                "Create or fully overwrite a text file inside the workspace. Reading an existing file first is required.",
                json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "content": {"type": "string"}
                    },
                    "required": ["path", "content"]
                }),
            ),
            ToolSpec::function(
                "edit",
                "Replace one exact, unique string in a file after reading it. Fails if the old string is missing or appears more than once.",
                json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "old": {"type": "string", "description": "Exact text to replace; must be unique"},
                        "new": {"type": "string", "description": "Replacement text"}
                    },
                    "required": ["path", "old", "new"]
                }),
            ),
            ToolSpec::function(
                "bash",
                "Run a shell command in the workspace. Returns the last 20KB of combined output and the exit code. Prefer this for tests and builds.",
                json!({
                    "type": "object",
                    "properties": {
                        "command": {"type": "string"},
                        "timeout_secs": {"type": "integer", "minimum": 1, "maximum": BASH_MAX_TIMEOUT}
                    },
                    "required": ["command"]
                }),
            ),
            ToolSpec::function(
                "grep",
                "Search text files in the workspace with a regular expression. Respects .gitignore, skips binaries and target/.git, and caps results.",
                json!({
                    "type": "object",
                    "properties": {
                        "pattern": {"type": "string", "description": "Rust regex"},
                        "path": {"type": "string", "description": "File or directory, defaults to the workspace root"},
                        "include": {"type": "string", "description": "Optional glob such as *.rs"},
                        "max_results": {"type": "integer", "minimum": 1, "maximum": SEARCH_MAX_RESULTS}
                    },
                    "required": ["pattern"]
                }),
            ),
            ToolSpec::function(
                "glob",
                "List files in the workspace matching a glob pattern, for example **/*.rs. Respects .gitignore and skips target/.git.",
                json!({
                    "type": "object",
                    "properties": {
                        "pattern": {"type": "string"},
                        "max_results": {"type": "integer", "minimum": 1, "maximum": SEARCH_MAX_RESULTS}
                    },
                    "required": ["pattern"]
                }),
            ),
        ]
    }

    pub async fn execute(&self, name: &str, input: &Value) -> ToolResult {
        let result = match name {
            "read" => self.read(input),
            "write" => self.write(input),
            "edit" => self.edit(input),
            "grep" => self.grep(input),
            "glob" => self.glob(input),
            "bash" => self.bash(input).await,
            other => Err(anyhow::anyhow!("unknown tool {other:?}")),
        };

        match result {
            Ok(output) => ToolResult { ok: true, output },
            Err(error) => ToolResult {
                ok: false,
                output: format!("error: {error:#}"),
            },
        }
    }

    fn read(&self, input: &Value) -> Result<String> {
        let path = self.resolve_existing(input_string(input, "path")?)?;
        let text = fs::read_to_string(&path)
            .with_context(|| format!("read {}", self.display_path(&path)))?;
        self.mark_observed(&path);

        let offset = input
            .get("offset")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .max(1) as usize;
        let limit = input
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(READ_MAX_LINES as u64)
            .clamp(1, READ_MAX_LINES as u64) as usize;

        let lines = text.lines().collect::<Vec<_>>();
        let total = lines.len();
        let start = offset.saturating_sub(1).min(total);
        let mut output = String::new();
        let mut shown = 0_usize;

        for (index, line) in lines.iter().enumerate().skip(start).take(limit) {
            let rendered = format!("{:>5}  {line}\n", index + 1);
            if output.len() + rendered.len() > READ_MAX_BYTES {
                break;
            }
            output.push_str(&rendered);
            shown += 1;
        }

        if shown == 0 {
            output.push_str(&format!("(no lines; file has {total} lines)\n"));
        } else if start + shown < total {
            output.push_str(&format!(
                "... ({} more lines; use offset to continue)\n",
                total - (start + shown)
            ));
        }

        Ok(output)
    }

    fn write(&self, input: &Value) -> Result<String> {
        let path = self.resolve_for_write(input_string(input, "path")?)?;
        let content = input_string(input, "content")?;

        if path.exists() {
            if path.is_dir() {
                bail!("{} is a directory", self.display_path(&path));
            }
            if !self.is_observed(&path) {
                bail!("read {} before writing it", self.display_path(&path));
            }
        }

        let parent = path.parent().context("path has no parent")?;
        if !parent.exists() {
            bail!(
                "parent directory does not exist: {}",
                self.display_path(parent)
            );
        }

        fs::write(&path, content).with_context(|| format!("write {}", self.display_path(&path)))?;
        self.mark_observed(&path);

        Ok(format!(
            "wrote {} bytes to {}",
            content.len(),
            self.display_path(&path)
        ))
    }

    fn edit(&self, input: &Value) -> Result<String> {
        let path = self.resolve_existing(input_string(input, "path")?)?;
        if !self.is_observed(&path) {
            bail!("read {} before editing it", self.display_path(&path));
        }

        let old = input_string(input, "old")?;
        let new = input_string(input, "new")?;
        if old.is_empty() {
            bail!("old string must not be empty");
        }

        let text = fs::read_to_string(&path)
            .with_context(|| format!("read {}", self.display_path(&path)))?;
        let matches = text.match_indices(old).count();
        match matches {
            0 => bail!("old string not found in {}", self.display_path(&path)),
            1 => {}
            count => bail!(
                "old string is not unique in {} ({count} matches); include more context",
                self.display_path(&path)
            ),
        }

        let updated = text.replacen(old, new, 1);
        fs::write(&path, &updated)
            .with_context(|| format!("write {}", self.display_path(&path)))?;
        self.mark_observed(&path);

        let diff = TextDiff::from_lines(&text, &updated)
            .unified_diff()
            .context_radius(3)
            .header("before", "after")
            .to_string();
        let preview: String = diff.chars().take(4_000).collect();

        Ok(format!(
            "edited {} (replaced {} bytes with {} bytes)\n{}",
            self.display_path(&path),
            old.len(),
            new.len(),
            preview
        ))
    }

    async fn bash(&self, input: &Value) -> Result<String> {
        let command = input_string(input, "command")?;
        if command.trim().is_empty() {
            bail!("command must not be empty");
        }

        let timeout_secs = input
            .get("timeout_secs")
            .and_then(Value::as_u64)
            .unwrap_or(BASH_DEFAULT_TIMEOUT)
            .clamp(1, BASH_MAX_TIMEOUT);

        let mut child_command = Command::new("bash");
        child_command
            .arg("-c")
            .arg(command)
            .current_dir(&self.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(unix)]
        child_command.process_group(0);

        let mut child = child_command.spawn().context("spawn bash")?;
        let pid = child.id().context("bash process id")?;
        let _guard = ProcessGroupGuard::new(pid);

        let stdout = child.stdout.take().context("take stdout")?;
        let stderr = child.stderr.take().context("take stderr")?;
        let stdout_task = tokio::spawn(read_capped(stdout, BASH_CAPTURE_BYTES));
        let stderr_task = tokio::spawn(read_capped(stderr, BASH_CAPTURE_BYTES));

        let status = match timeout(Duration::from_secs(timeout_secs), child.wait()).await {
            Ok(result) => result.context("wait for bash")?,
            Err(_) => {
                kill_group(pid);
                let _ = child.kill().await;
                let _ = child.wait().await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                bail!("command timed out after {timeout_secs}s");
            }
        };

        let (stdout, stdout_truncated) = stdout_task
            .await
            .context("join stdout")?
            .context("read stdout")?;
        let (stderr, stderr_truncated) = stderr_task
            .await
            .context("join stderr")?
            .context("read stderr")?;

        let mut combined = String::new();
        if !stdout.is_empty() {
            combined.push_str("stdout:\n");
            combined.push_str(&String::from_utf8_lossy(&stdout));
        }
        if !stderr.is_empty() {
            if !combined.is_empty() {
                combined.push('\n');
            }
            combined.push_str("stderr:\n");
            combined.push_str(&String::from_utf8_lossy(&stderr));
        }
        if combined.is_empty() {
            combined.push_str("(no output)\n");
        }

        let status = status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "signal".into());

        if stdout_truncated || stderr_truncated || combined.chars().count() > BASH_MAX_BYTES {
            let path = temp_output_path(pid);
            fs::write(&path, &combined)
                .with_context(|| format!("write full bash output {}", path.display()))?;
            let (body, _) = tail_chars(&combined, BASH_MAX_BYTES);
            let qualifier = if stdout_truncated || stderr_truncated {
                "captured output (capped at 8MB)"
            } else {
                "full output"
            };
            return Ok(format!(
                "exit: {status}\n{body}\n... ({qualifier}: {})\n",
                path.display()
            ));
        }

        Ok(format!("exit: {status}\n{combined}"))
    }

    fn grep(&self, input: &Value) -> Result<String> {
        let pattern = input_string(input, "pattern")?;
        if pattern.is_empty() {
            bail!("pattern must not be empty");
        }
        let regex = Regex::new(pattern).with_context(|| format!("invalid regex {pattern:?}"))?;
        let root = match input.get("path").and_then(Value::as_str) {
            Some(path) if !path.trim().is_empty() => self.resolve_existing(path)?,
            _ => self.cwd.clone(),
        };
        let include = match input.get("include").and_then(Value::as_str) {
            Some(glob) if !glob.trim().is_empty() => Some(
                GlobBuilder::new(glob)
                    .literal_separator(false)
                    .build()
                    .with_context(|| format!("invalid include glob {glob:?}"))?
                    .compile_matcher(),
            ),
            _ => None,
        };
        let max = input
            .get("max_results")
            .and_then(Value::as_u64)
            .unwrap_or(100)
            .clamp(1, SEARCH_MAX_RESULTS as u64) as usize;

        let walker = WalkBuilder::new(&root)
            .standard_filters(true)
            .require_git(false)
            .filter_entry(|entry| {
                let name = entry.file_name().to_string_lossy();
                name != ".git" && name != "target" && name != "node_modules"
            })
            .build();

        let mut matches = Vec::new();
        let mut truncated = false;

        'outer: for entry in walker {
            let entry = entry.context("walk workspace")?;
            if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                continue;
            }
            let path = entry.path();
            let relative = self.display_path(path);
            if let Some(matcher) = &include {
                if !matcher.is_match(&relative) {
                    continue;
                }
            }
            if entry.metadata().map(|meta| meta.len()).unwrap_or(0) > 2_000_000 {
                continue;
            }
            let Ok(text) = fs::read_to_string(path) else {
                continue;
            };
            if text.contains('\0') {
                continue;
            }

            for (index, line) in text.lines().enumerate() {
                if !regex.is_match(line) {
                    continue;
                }
                let preview: String = line.chars().take(500).collect();
                matches.push(format!("{relative}:{}: {preview}", index + 1));
                if matches.len() >= max {
                    truncated = true;
                    break 'outer;
                }
            }
        }

        if matches.is_empty() {
            return Ok(format!("no matches for {pattern:?}"));
        }

        let mut output = matches.join("\n");
        if truncated {
            output.push_str(&format!("\n... (stopped at {max} matches)"));
        }
        Ok(output)
    }

    fn glob(&self, input: &Value) -> Result<String> {
        let pattern = input_string(input, "pattern")?;
        if pattern.trim().is_empty() {
            bail!("pattern must not be empty");
        }
        let matcher = GlobBuilder::new(pattern)
            .literal_separator(false)
            .build()
            .with_context(|| format!("invalid glob {pattern:?}"))?
            .compile_matcher();
        let max = input
            .get("max_results")
            .and_then(Value::as_u64)
            .unwrap_or(SEARCH_MAX_RESULTS as u64)
            .clamp(1, SEARCH_MAX_RESULTS as u64) as usize;

        let walker = WalkBuilder::new(&self.cwd)
            .standard_filters(true)
            .require_git(false)
            .filter_entry(|entry| {
                let name = entry.file_name().to_string_lossy();
                name != ".git" && name != "target" && name != "node_modules"
            })
            .build();

        let mut paths = Vec::new();
        for entry in walker {
            let entry = entry.context("walk workspace")?;
            if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                continue;
            }
            let relative = self.display_path(entry.path());
            if matcher.is_match(&relative) {
                paths.push(relative);
                if paths.len() >= max {
                    break;
                }
            }
        }

        if paths.is_empty() {
            return Ok(format!("no files match {pattern:?}"));
        }
        paths.sort();
        Ok(paths.join("\n"))
    }

    fn mark_observed(&self, path: &Path) {
        self.observed
            .lock()
            .expect("observed lock")
            .insert(path.to_path_buf());
    }

    fn is_observed(&self, path: &Path) -> bool {
        self.observed.lock().expect("observed lock").contains(path)
    }

    fn resolve_existing(&self, raw: &str) -> Result<PathBuf> {
        let candidate = self.candidate(raw);
        let canonical = candidate
            .canonicalize()
            .with_context(|| format!("resolve {}", raw))?;
        self.ensure_inside(&canonical)?;
        Ok(canonical)
    }

    fn resolve_for_write(&self, raw: &str) -> Result<PathBuf> {
        let candidate = self.candidate(raw);
        if candidate.exists() {
            let canonical = candidate
                .canonicalize()
                .with_context(|| format!("resolve {}", raw))?;
            self.ensure_inside(&canonical)?;
            return Ok(canonical);
        }

        let parent = candidate.parent().context("path has no parent")?;
        let parent = parent
            .canonicalize()
            .with_context(|| format!("resolve parent of {raw}"))?;
        self.ensure_inside(&parent)?;
        let name = candidate.file_name().context("path has no file name")?;
        Ok(parent.join(name))
    }

    fn candidate(&self, raw: &str) -> PathBuf {
        let path = Path::new(raw);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        }
    }

    fn ensure_inside(&self, path: &Path) -> Result<()> {
        if !path.starts_with(&self.cwd) {
            bail!("path escapes the workspace: {}", path.display());
        }
        Ok(())
    }

    fn display_path(&self, path: &Path) -> String {
        path.strip_prefix(&self.cwd)
            .unwrap_or(path)
            .display()
            .to_string()
    }
}

struct ProcessGroupGuard {
    pid: u32,
}

impl ProcessGroupGuard {
    fn new(pid: u32) -> Self {
        Self { pid }
    }
}

impl Drop for ProcessGroupGuard {
    fn drop(&mut self) {
        kill_group(self.pid);
    }
}

fn kill_group(pid: u32) {
    #[cfg(unix)]
    {
        if pid > 0 {
            // Negative pid targets the process group created by process_group(0).
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
    }
}

async fn read_capped<R: AsyncRead + Unpin>(
    mut reader: R,
    cap: usize,
) -> std::io::Result<(Vec<u8>, bool)> {
    let mut data = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut truncated = false;

    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        if data.len() < cap {
            let take = (cap - data.len()).min(read);
            data.extend_from_slice(&buffer[..take]);
            if take < read {
                truncated = true;
            }
        } else {
            truncated = true;
        }
    }

    Ok((data, truncated))
}

fn temp_output_path(pid: u32) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("ni-bash-{pid}-{stamp}.log"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    pub ok: bool,
    pub output: String,
}

fn input_string<'a>(input: &'a Value, key: &str) -> Result<&'a str> {
    input
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("missing string argument {key:?}"))
}

fn tail_chars(value: &str, max_chars: usize) -> (String, bool) {
    let count = value.chars().count();
    if count <= max_chars {
        return (value.to_string(), false);
    }
    (value.chars().skip(count - max_chars).collect(), true)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_workspace(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("ni-tools-{tag}-{stamp}-{sequence}"));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create workspace");
        directory
    }

    #[tokio::test]
    async fn read_write_edit_round_trip() {
        let directory = temp_workspace("roundtrip");
        fs::write(directory.join("a.txt"), "hello\nworld\n").expect("fixture");
        let tools = Toolbox::new(&directory).expect("toolbox");

        let read = tools.execute("read", &json!({"path": "a.txt"})).await;
        assert!(read.ok, "{}", read.output);
        assert!(read.output.contains("hello"));
        assert!(read.output.contains("world"));

        let write = tools
            .execute("write", &json!({"path": "b.txt", "content": "new file"}))
            .await;
        assert!(write.ok, "{}", write.output);
        assert_eq!(
            fs::read_to_string(directory.join("b.txt")).expect("read"),
            "new file"
        );

        let edit = tools
            .execute(
                "edit",
                &json!({"path": "a.txt", "old": "world", "new": "ni"}),
            )
            .await;
        assert!(edit.ok, "{}", edit.output);
        assert_eq!(
            fs::read_to_string(directory.join("a.txt")).expect("read"),
            "hello\nni\n"
        );

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[tokio::test]
    async fn existing_file_must_be_read_before_write() {
        let directory = temp_workspace("read-first");
        fs::write(directory.join("a.txt"), "old").expect("fixture");
        let tools = Toolbox::new(&directory).expect("toolbox");

        let rejected = tools
            .execute("write", &json!({"path": "a.txt", "content": "new"}))
            .await;
        assert!(!rejected.ok);
        assert!(rejected.output.contains("read a.txt"));

        let read = tools.execute("read", &json!({"path": "a.txt"})).await;
        assert!(read.ok);
        let written = tools
            .execute("write", &json!({"path": "a.txt", "content": "new"}))
            .await;
        assert!(written.ok, "{}", written.output);

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[tokio::test]
    async fn edit_rejects_ambiguous_match() {
        let directory = temp_workspace("ambiguous");
        fs::write(directory.join("a.txt"), "same same").expect("fixture");
        let tools = Toolbox::new(&directory).expect("toolbox");
        tools.execute("read", &json!({"path": "a.txt"})).await;

        let result = tools
            .execute("edit", &json!({"path": "a.txt", "old": "same", "new": "x"}))
            .await;
        assert!(!result.ok);
        assert!(result.output.contains("not unique"));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[tokio::test]
    async fn rejects_paths_outside_workspace() {
        let directory = temp_workspace("escape");
        let outside = directory
            .parent()
            .expect("parent")
            .join("outside-secret.txt");
        fs::write(&outside, "secret").expect("fixture");
        let tools = Toolbox::new(&directory).expect("toolbox");

        let result = tools
            .execute("read", &json!({"path": "../outside-secret.txt"}))
            .await;
        assert!(!result.ok);
        assert!(result.output.contains("outside") || result.output.contains("escapes"));

        let _ = fs::remove_file(outside);
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[tokio::test]
    async fn grep_and_glob_respect_include_and_gitignore() {
        let directory = temp_workspace("search");
        fs::create_dir_all(directory.join("src")).expect("src dir");
        fs::write(directory.join("src/a.rs"), "fn needle() {}\n").expect("fixture");
        fs::write(directory.join("src/b.txt"), "needle\n").expect("fixture");
        fs::write(directory.join(".gitignore"), "ignored.rs\n").expect("gitignore");
        fs::write(directory.join("ignored.rs"), "needle\n").expect("fixture");
        let tools = Toolbox::new(&directory).expect("toolbox");

        let grep = tools
            .execute("grep", &json!({"pattern": "needle", "include": "*.rs"}))
            .await;
        assert!(grep.ok, "{}", grep.output);
        assert!(grep.output.contains("src/a.rs:1:"));
        assert!(!grep.output.contains("b.txt"));
        assert!(!grep.output.contains("ignored.rs"));

        let glob = tools.execute("glob", &json!({"pattern": "*.rs"})).await;
        assert!(glob.ok, "{}", glob.output);
        assert!(glob.output.contains("src/a.rs"));
        assert!(!glob.output.contains("ignored.rs"));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[tokio::test]
    async fn bash_timeout_kills_the_process_group() {
        let directory = temp_workspace("timeout");
        let tools = Toolbox::new(&directory).expect("toolbox");
        let started = Instant::now();

        let result = tools
            .execute("bash", &json!({"command": "sleep 5", "timeout_secs": 1}))
            .await;

        assert!(!result.ok);
        assert!(result.output.contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(4));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[tokio::test]
    async fn bash_returns_output_and_exit_code() {
        let directory = temp_workspace("bash");
        let tools = Toolbox::new(&directory).expect("toolbox");

        let result = tools
            .execute("bash", &json!({"command": "printf 'hello'; exit 3"}))
            .await;
        assert!(result.ok);
        assert!(result.output.contains("hello"));
        assert!(result.output.contains("exit: 3"));

        fs::remove_dir_all(directory).expect("cleanup");
    }
}
