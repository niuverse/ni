//! Read-only parallel subagents with independent session logs.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use futures_util::future::join_all;
use niucode_core::{now_ms, Event, SessionLog, ToolCall, Usage};
use niucode_provider::Provider;
use niucode_tools::{NetworkMode, SandboxMode, SandboxPolicy, ToolResult, Toolbox};
use serde_json::Value;

use crate::{run, AgentConfig, ApprovalMode};

const MAX_TASKS: usize = 4;
const MAX_STEPS: usize = 12;
const MAX_TIMEOUT_SECS: u64 = 600;
const MAX_PROMPT_CHARS: usize = 8_000;
const MAX_SUMMARY_CHARS: usize = 2_000;
const MAX_EVENT_TASK_CHARS: usize = 4_000;
const SUBAGENT_CONTEXT_BUDGET: usize = 16_000;
const SUBAGENT_TOOLS: [&str; 3] = ["read", "grep", "glob"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubagentProfile {
    Explore,
    Worker,
    Inherit,
}

impl SubagentProfile {
    fn as_str(self) -> &'static str {
        match self {
            Self::Explore => "explore",
            Self::Worker => "worker",
            Self::Inherit => "inherit",
        }
    }
}

struct TaskSpec {
    prompt: String,
    profile: SubagentProfile,
    write_scope: Option<Vec<String>>,
    worktree: bool,
    max_steps: usize,
    timeout_secs: u64,
}

struct SubagentOutcome {
    ok: bool,
    summary: String,
    session: std::path::PathBuf,
}

/// Execute the `subagent` tool. Errors become an ordinary failed tool result.
pub async fn run_tool(
    provider: &dyn Provider,
    parent_session: &mut SessionLog,
    parent_config: &AgentConfig,
    call: &ToolCall,
) -> ToolResult {
    match run_tasks(provider, parent_session, parent_config, call).await {
        Ok(outcomes) => {
            let ok = outcomes.iter().all(|outcome| outcome.ok);
            ToolResult {
                ok,
                output: render_outcomes(&outcomes),
            }
        }
        Err(error) => ToolResult {
            ok: false,
            output: format!("error: {error:#}"),
        },
    }
}

async fn run_tasks(
    provider: &dyn Provider,
    parent_session: &mut SessionLog,
    parent_config: &AgentConfig,
    call: &ToolCall,
) -> Result<Vec<SubagentOutcome>> {
    let tasks = parse_tasks(&call.input)?;
    if tasks
        .iter()
        .any(|task| task.profile == SubagentProfile::Inherit)
        && !parent_config.allow_subagent_inheritance
    {
        bail!(
            "subagent profile \"inherit\" requires --allow-subagent-inheritance (explicit opt-in)"
        );
    }
    let directory = parent_session
        .path()
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{}.subagents", parent_session.id()));
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("create subagent session directory {}", directory.display()))?;

    let mut jobs = Vec::with_capacity(tasks.len());
    for (index, task) in tasks.into_iter().enumerate() {
        let id = format!("{}.{}", call.id, index + 1);
        let mut config = subagent_config(parent_config, &task);
        let worktree = if task.worktree {
            match Worktree::create(parent_config, &id) {
                Some(worktree) => {
                    config.working_dir = worktree.path.clone();
                    config.write_lock = None;
                    Some(worktree)
                }
                None => {
                    eprintln!(
                        "warning: worktree unavailable; worker {id} uses the shared workspace"
                    );
                    None
                }
            }
        } else {
            None
        };
        let mut session = SessionLog::create(&directory)?;
        session.append(&Event::SessionStart {
            id: id.clone(),
            cwd: parent_config.working_dir.display().to_string(),
            model: parent_config.model.clone(),
            provider: parent_config.provider.clone(),
            ts: now_ms(),
        })?;
        session.append(&Event::Policy {
            sandbox: config.sandbox.mode.as_str().into(),
            approval: config.approval.as_str().into(),
            network: config.sandbox.network.as_str().into(),
            backend: config.sandbox.backend().into(),
            ts: now_ms(),
        })?;
        let session_path = session.path().to_path_buf();
        parent_session.append(&Event::SubagentSpawn {
            id: id.clone(),
            call_id: call.id.clone(),
            task: task.prompt.chars().take(MAX_EVENT_TASK_CHARS).collect(),
            profile: task.profile.as_str().to_string(),
            session: session_path.display().to_string(),
            max_steps: task.max_steps as u64,
            timeout_secs: task.timeout_secs,
            context_budget: config.context.budget_tokens as u64,
            ts: now_ms(),
        })?;
        jobs.push((id, task, session, session_path, config, worktree));
    }

    let futures = jobs.into_iter().map(
        |(id, task, mut session, session_path, config, worktree)| async move {
            let timeout_secs = task.timeout_secs;
            let result = tokio::time::timeout(
                Duration::from_secs(timeout_secs),
                run(
                    provider,
                    &mut session,
                    &config,
                    &task.prompt,
                    &mut |_| false,
                    &mut |_| {},
                ),
            )
            .await;

            let (ok, summary, turns, usage) = match result {
                Ok(Ok(outcome)) => (
                    true,
                    outcome.final_text,
                    outcome.turns as u64,
                    outcome.usage,
                ),
                Ok(Err(error)) => {
                    let _ = session.append(&Event::SessionEnd { ts: now_ms() });
                    (false, format!("error: {error:#}"), 0, Usage::default())
                }
                Err(_) => {
                    let _ = session.append(&Event::SessionEnd { ts: now_ms() });
                    (
                        false,
                        format!("timeout after {timeout_secs}s"),
                        0,
                        Usage::default(),
                    )
                }
            };

            let patch = worktree.and_then(Worktree::finish);
            let summary = match patch {
                Some(patch) => format!(
                    "worktree patch: {}\n\n{}",
                    patch.display(),
                    summarize(&summary)
                ),
                None => summarize(&summary),
            };
            (id, task.prompt, session_path, ok, summary, turns, usage)
        },
    );

    let results = join_all(futures).await;
    let mut outcomes = Vec::with_capacity(results.len());
    for (id, _task, session, ok, summary, turns, usage) in results {
        parent_session.append(&Event::SubagentResult {
            id: id.clone(),
            call_id: call.id.clone(),
            ok,
            summary: summary.clone(),
            session: session.display().to_string(),
            turns,
            usage: usage.clone(),
            ts: now_ms(),
        })?;
        outcomes.push(SubagentOutcome {
            ok,
            summary,
            session,
        });
    }
    Ok(outcomes)
}

struct Worktree {
    path: PathBuf,
    repo: PathBuf,
    patch: PathBuf,
}

impl Worktree {
    fn create(parent: &AgentConfig, id: &str) -> Option<Self> {
        let root = parent.worktree_root.as_ref()?;
        std::fs::create_dir_all(root).ok()?;
        let repo = git_output(&parent.working_dir, &["rev-parse", "--show-toplevel"])?;
        let repo = PathBuf::from(repo.trim());
        let path = root.join(format!("{}-{}", id.replace('/', "_"), std::process::id()));
        let patch = root.join(format!("{}.patch", id.replace('/', "_")));
        let _ = std::fs::remove_dir_all(&path);
        let status = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["worktree", "add", "--detach"])
            .arg(&path)
            .arg("HEAD")
            .status()
            .ok()?;
        if !status.success() {
            return None;
        }
        Some(Self { path, repo, patch })
    }

    fn finish(self) -> Option<PathBuf> {
        let _ = Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .args(["add", "-N", "."])
            .status();
        let diff = Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .args(["diff", "--binary", "--no-color"])
            .output()
            .ok()?;
        let _ = Command::new("git")
            .arg("-C")
            .arg(&self.repo)
            .args(["worktree", "remove", "--force"])
            .arg(&self.path)
            .status();
        let _ = std::fs::remove_dir_all(&self.path);
        if !diff.status.success() || diff.stdout.is_empty() {
            return None;
        }
        std::fs::write(&self.patch, diff.stdout).ok()?;
        Some(self.patch)
    }
}

fn git_output(directory: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn parse_tasks(input: &Value) -> Result<Vec<TaskSpec>> {
    let tasks = input
        .get("tasks")
        .and_then(Value::as_array)
        .context("missing tasks array")?;
    if tasks.is_empty() {
        bail!("tasks must not be empty");
    }
    if tasks.len() > MAX_TASKS {
        bail!("at most {MAX_TASKS} subagent tasks per call");
    }

    tasks
        .iter()
        .enumerate()
        .map(|(index, task)| {
            let prompt = task
                .get("prompt")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|prompt| !prompt.is_empty())
                .with_context(|| format!("task {} is missing a prompt", index + 1))?;
            if prompt.chars().count() > MAX_PROMPT_CHARS {
                bail!("task {} prompt is too long", index + 1);
            }
            let profile = match task
                .get("profile")
                .and_then(Value::as_str)
                .unwrap_or("explore")
            {
                "explore" => SubagentProfile::Explore,
                "worker" => SubagentProfile::Worker,
                "inherit" => SubagentProfile::Inherit,
                other => bail!("task {} has unknown profile {other:?}", index + 1),
            };
            let write_scope = task
                .get("write_scope")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .filter(|values| !values.is_empty());
            if profile == SubagentProfile::Explore && write_scope.is_some() {
                bail!(
                    "task {} is explore-only and cannot declare write_scope",
                    index + 1
                );
            }
            let worktree = task
                .get("worktree")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if worktree && profile != SubagentProfile::Worker {
                bail!(
                    "task {}: worktree is only supported for worker profile",
                    index + 1
                );
            }
            let max_steps = task
                .get("max_steps")
                .and_then(Value::as_u64)
                .map_or(6, |steps| steps.clamp(1, MAX_STEPS as u64) as usize);
            let timeout_secs = task
                .get("timeout_secs")
                .and_then(Value::as_u64)
                .map_or(120, |secs| secs.clamp(1, MAX_TIMEOUT_SECS));
            Ok(TaskSpec {
                prompt: prompt.to_string(),
                profile,
                write_scope,
                worktree,
                max_steps,
                timeout_secs,
            })
        })
        .collect()
}

fn subagent_config(parent: &AgentConfig, task: &TaskSpec) -> AgentConfig {
    let mut config = AgentConfig::new(parent.model.clone(), parent.working_dir.clone());
    config.provider = parent.provider.clone();
    config.max_steps = task.max_steps.clamp(1, MAX_STEPS);
    config.context.budget_tokens = parent
        .context
        .budget_tokens
        .clamp(1_000, SUBAGENT_CONTEXT_BUDGET);
    config.context.compact_at_percent = 75;
    config.context.keep_recent_turns = 1;
    config.extension_host = parent.extension_host.clone();
    config.write_lock = parent.write_lock.clone();
    config.read_roots = parent.read_roots.clone();

    let mut allowed = match task.profile {
        SubagentProfile::Explore => SUBAGENT_TOOLS
            .iter()
            .map(|name| (*name).to_string())
            .collect::<Vec<_>>(),
        SubagentProfile::Worker => ["read", "write", "edit", "grep", "glob", "bash"]
            .iter()
            .map(|name| (*name).to_string())
            .collect::<Vec<_>>(),
        // Replaced by the parent tool surface in the Inherit branch below.
        SubagentProfile::Inherit => Vec::new(),
    };
    if let Some(host) = &parent.extension_host {
        allowed.extend(host.read_only_tools());
    }
    allowed.sort();
    allowed.dedup();
    config.allowed_tools = Some(allowed);

    match task.profile {
        SubagentProfile::Explore => {
            config.system_prompt = format!(
                "You are a read-only exploration subagent working inside {}.\n\
                 You may use read, grep, glob, and read-only external tools only. \n\
                 Do not modify files and do not run shell commands.\n\
                 Answer with a concise summary that includes file paths and line numbers. \n\
                 The parent agent sees only this summary; the full transcript stays in your own session log.\n\
                 Do not ask questions.",
                parent.working_dir.display()
            );
            config.sandbox = SandboxPolicy::new(SandboxMode::ReadOnly, NetworkMode::Off);
            config.approval = ApprovalMode::Never;
            config.allow_unsandboxed = false;
        }
        SubagentProfile::Worker => {
            config.system_prompt = format!(
                "You are a Worker subagent working inside {}.\n\
                 You may use read, write, edit, grep, glob, bash, and read-only external tools.\n\
                 Writes are serialized by a shared workspace write lock; respect the declared write scope.\n\
                 Do not delegate to another subagent. Keep changes scoped and verify them before returning.\n\
                 The parent agent sees only your final summary; the full transcript stays in your own session log.\n\
                 Do not ask questions.",
                parent.working_dir.display()
            );
            config.sandbox = parent.sandbox;
            config.approval = parent.approval;
            config.allow_unsandboxed = false;
            config.write_scope = task.write_scope.clone();
        }
        SubagentProfile::Inherit => {
            config.system_prompt = format!(
                "You are an explicitly inherited subagent working inside {}.\n\
                 You inherit the parent's tool surface and policy, except that nested subagents are disabled.\n\
                 Respect the parent's sandbox, approval, write lock, and declared write scope.\n\
                 The parent agent sees only your final summary; the full transcript stays in your own session log.\n\
                 Do not ask questions.",
                parent.working_dir.display()
            );
            let mut tools = match &parent.allowed_tools {
                Some(tools) => tools.clone(),
                None => {
                    let mut names = Toolbox::specs()
                        .into_iter()
                        .map(|tool| tool.function.name)
                        .collect::<Vec<_>>();
                    if let Some(host) = &parent.extension_host {
                        names.extend(host.specs().into_iter().map(|tool| tool.function.name));
                    }
                    names
                }
            };
            tools.retain(|name| name != "subagent");
            tools.sort();
            tools.dedup();
            config.allowed_tools = Some(tools);
            config.sandbox = parent.sandbox;
            config.approval = parent.approval;
            config.allow_unsandboxed = parent.allow_unsandboxed;
            config.plan_mode = parent.plan_mode;
            config.write_scope = task.write_scope.clone();
        }
    }
    config
}

fn summarize(text: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        return "(no final summary)".to_string();
    }
    let mut summary = text.chars().take(MAX_SUMMARY_CHARS).collect::<String>();
    if text.chars().count() > MAX_SUMMARY_CHARS {
        summary.push('…');
    }
    summary
}

fn render_outcomes(outcomes: &[SubagentOutcome]) -> String {
    let mut rendered = format!("subagent: {} task(s)\n", outcomes.len());
    for (index, outcome) in outcomes.iter().enumerate() {
        rendered.push_str(&format!(
            "[{}] {} · {}\n    log: {}\n",
            index + 1,
            if outcome.ok { "ok" } else { "error" },
            outcome.summary,
            outcome.session.display()
        ));
    }
    rendered
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_tasks_and_clamps_budgets() {
        let tasks = parse_tasks(&json!({
            "tasks": [
                {"prompt": "inspect a", "max_steps": 99, "timeout_secs": 9999},
                {"prompt": "inspect b"}
            ]
        }))
        .expect("tasks");
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].max_steps, MAX_STEPS);
        assert_eq!(tasks[0].timeout_secs, MAX_TIMEOUT_SECS);
        assert_eq!(tasks[1].max_steps, 6);
        assert_eq!(tasks[1].timeout_secs, 120);
    }

    #[test]
    fn rejects_empty_and_oversized_task_lists() {
        assert!(parse_tasks(&json!({"tasks": []})).is_err());
        let tasks = (0..5)
            .map(|index| json!({"prompt": format!("task {index}")}))
            .collect::<Vec<_>>();
        assert!(parse_tasks(&json!({"tasks": tasks})).is_err());
        assert!(parse_tasks(&json!({"tasks": [{}]})).is_err());
    }

    #[test]
    fn summaries_are_bounded() {
        let long = "x".repeat(MAX_SUMMARY_CHARS + 10);
        let summary = summarize(&long);
        assert_eq!(summary.chars().count(), MAX_SUMMARY_CHARS + 1);
        assert!(summary.ends_with('…'));
        assert_eq!(summarize("  "), "(no final summary)");
    }
}

#[cfg(test)]
mod worktree_tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("niucode-worktree-{tag}-{stamp}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("temp");
        path
    }

    fn git(directory: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(directory)
            .args(args)
            .status()
            .expect("git");
        assert!(status.success(), "git {args:?}");
    }

    fn repo(tag: &str) -> (PathBuf, AgentConfig) {
        let directory = temp_dir(tag);
        git(&directory, &["init", "-q"]);
        git(
            &directory,
            &["config", "user.email", "test@example.invalid"],
        );
        git(&directory, &["config", "user.name", "test"]);
        std::fs::write(directory.join("file.txt"), "base\n").expect("base");
        git(&directory, &["add", "."]);
        git(&directory, &["commit", "-qm", "base"]);
        let root = directory.join("worktrees");
        let mut config = AgentConfig::new("test", directory.clone());
        config.worktree_root = Some(root);
        (directory, config)
    }

    #[test]
    fn two_workers_get_independent_worktrees_and_patches() {
        let (directory, config) = repo("two");
        let first = Worktree::create(&config, "worker-a").expect("first worktree");
        let second = Worktree::create(&config, "worker-b").expect("second worktree");
        assert_ne!(first.path, second.path);
        std::fs::write(first.path.join("file.txt"), "first\n").expect("first write");
        std::fs::write(second.path.join("file.txt"), "second\n").expect("second write");
        let first_patch = first.finish().expect("first patch");
        let second_patch = second.finish().expect("second patch");
        assert!(std::fs::read_to_string(first_patch)
            .expect("first patch text")
            .contains("first"));
        assert!(std::fs::read_to_string(second_patch)
            .expect("second patch text")
            .contains("second"));
        assert_eq!(
            std::fs::read_to_string(directory.join("file.txt")).expect("parent file"),
            "base\n",
            "worker worktree leaked into parent"
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn non_git_workspace_falls_back_to_shared_workdir() {
        let directory = temp_dir("non-git");
        let mut config = AgentConfig::new("test", directory.clone());
        config.worktree_root = Some(directory.join("worktrees"));
        assert!(Worktree::create(&config, "fallback").is_none());
        std::fs::remove_dir_all(directory).expect("cleanup");
    }
}
