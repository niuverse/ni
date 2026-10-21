use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use futures_util::StreamExt;
use niucode_agent::{
    project_history, run_with_history, AgentConfig, AgentEvent, ApprovalMode, ApprovalRequest,
};
use niucode_auth::AuthStore;
use niucode_core::{now_ms, Event, SessionLog};
use niucode_extension::{discover_in, ExtensionHost};
use niucode_provider::{
    ChatMessage, CompletionRequest, OpenAiConfig, OpenAiProvider, Preset, Provider, ReplayProvider,
    StreamEvent, ThinkingMode,
};
use niucode_tools::{NetworkMode, SandboxMode, SandboxPolicy, Toolbox};
use provider_factory::ProviderFactory;
use serde_json::json;

use crate::theme::Theme;

mod completion;
mod composer;
mod details;
mod diff;
mod editor;
mod goal;
mod init;
mod live;
mod presentation;
mod provider_factory;
mod review;
mod session_ops;
mod skills;
mod slash;
mod terminal;
mod text_layout;
mod theme;
mod tui;
mod workspace;

#[derive(Debug, Parser)]
#[command(
    name = "niucode",
    version,
    about = "Minimal, observable coding agent; starts the TUI when no subcommand is given"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum WorktreeCommand {
    /// Apply a patch produced by a Worker worktree
    Apply {
        /// Patch path printed in the Worker summary
        #[arg(long)]
        patch: PathBuf,
        /// Only check whether the patch applies
        #[arg(long)]
        check: bool,
    },
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect and create append-only session logs
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    /// Replay a session offline and print per-turn projections
    Replay {
        /// Path to a .jsonl session file
        path: PathBuf,
    },
    /// Copy a session prefix into a new session
    Fork {
        /// Path to the source .jsonl session file
        path: PathBuf,
        /// Event id to fork at, for example e000012
        #[arg(long)]
        at: String,
        /// Directory for the new session file (defaults to the source directory)
        #[arg(long)]
        session_dir: Option<PathBuf>,
    },
    /// Apply a Worker worktree patch
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommand,
    },
    /// Inspect and test model providers
    Provider {
        #[command(subcommand)]
        command: ProviderCommand,
    },
    /// Store API keys in the OS keyring or a 0600 file
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Report or reclaim ~/.niucode storage (dry-run by default)
    Gc {
        /// Remove derived caches, old proposals, and old backups
        #[arg(long)]
        apply: bool,
        /// Also delete old sessions (requires --yes)
        #[arg(long)]
        prune_sessions: bool,
        /// Confirm destructive --prune-sessions
        #[arg(long)]
        yes: bool,
    },
    /// Run one coding task through the agent loop
    Exec {
        /// Task for the agent
        prompt: String,
        /// Provider preset: deepseek or openai
        #[arg(long, default_value = "deepseek")]
        preset: String,
        /// Override the preset model
        #[arg(long)]
        model: Option<String>,
        /// Maximum model turns per user request (0 = no hard cap)
        #[arg(long, default_value_t = 0)]
        max_steps: usize,
        /// Approximate context budget in tokens before compaction (0 = auto)
        #[arg(long, default_value_t = 0)]
        context_budget: usize,
        /// Recent user turns kept verbatim when compacting
        #[arg(long, default_value_t = 3)]
        keep_recent_turns: usize,
        /// Disable DeepSeek thinking mode
        #[arg(long)]
        no_thinking: bool,
        /// Working directory (defaults to cwd)
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Directory for the session JSONL (defaults to ~/.niucode/sessions)
        #[arg(long)]
        session_dir: Option<PathBuf>,
        /// Resume an existing session JSONL
        #[arg(long)]
        resume: Option<PathBuf>,
        /// Record normalized provider turns to a fixture JSON
        #[arg(long)]
        record: Option<PathBuf>,
        /// Replay normalized provider turns from a fixture JSON
        #[arg(long)]
        replay: Option<PathBuf>,
        /// Sandbox: read-only, workspace-write, or full-access
        #[arg(long, default_value = "workspace-write")]
        sandbox: String,
        /// Approval: ask, on-failure, or never
        #[arg(long, default_value = "on-failure")]
        approval: String,
        /// Network: on or off
        #[arg(long, default_value = "off")]
        network: String,
        /// Emit a stable JSONL event stream on stdout
        #[arg(long)]
        json: bool,
        /// Explicitly allow subagent profile=inherit
        #[arg(long)]
        allow_subagent_inheritance: bool,
        /// Restrict advertised and executable tools (comma-separated names)
        #[arg(long, value_delimiter = ',')]
        tools: Option<Vec<String>>,
        /// Completion condition; run the evaluator loop until met/impossible
        #[arg(long)]
        goal: Option<String>,
        /// Maximum actor rounds for --goal
        #[arg(long, default_value_t = 6)]
        max_goal_turns: usize,
    },
    /// Start the interactive linear TUI
    Tui {
        /// Provider preset: deepseek or openai
        #[arg(long, default_value = "deepseek")]
        preset: String,
        /// Override the preset model
        #[arg(long)]
        model: Option<String>,
        /// Approximate context budget in tokens before compaction (0 = auto)
        #[arg(long, default_value_t = 0)]
        context_budget: usize,
        /// Maximum model turns per user request (0 = no hard cap)
        #[arg(long, default_value_t = 0)]
        max_steps: usize,
        /// Recent user turns kept verbatim when compacting
        #[arg(long, default_value_t = 3)]
        keep_recent_turns: usize,
        /// Disable DeepSeek thinking mode
        #[arg(long)]
        no_thinking: bool,
        /// Working directory (defaults to cwd)
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Directory for the session JSONL (defaults to ~/.niucode/sessions)
        #[arg(long)]
        session_dir: Option<PathBuf>,
        /// Resume an existing session JSONL
        #[arg(long)]
        resume: Option<PathBuf>,
        /// Sandbox: read-only, workspace-write, or full-access
        #[arg(long, default_value = "workspace-write")]
        sandbox: String,
        /// Approval: ask, on-failure, or never
        #[arg(long, default_value = "on-failure")]
        approval: String,
        /// Network: on or off
        #[arg(long, default_value = "off")]
        network: String,
        /// Replay normalized provider turns from a fixture JSON
        #[arg(long)]
        replay: Option<PathBuf>,
        /// Start in read-only plan mode
        #[arg(long)]
        plan: bool,
        /// Explicitly allow subagent profile=inherit
        #[arg(long)]
        allow_subagent_inheritance: bool,
    },
}

#[derive(Debug, Subcommand)]
enum SessionCommand {
    /// Create a session and print its path
    New {
        /// Directory for the session file
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Print the events in a session file
    Show {
        /// Path to a .jsonl session file
        path: PathBuf,
    },
    /// List session files
    List {
        /// Directory containing session files
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum AuthCommand {
    /// Store an API key (hidden prompt, or --stdin for scripts)
    Set {
        /// Provider: deepseek or openai
        provider: String,
        /// Read one line from stdin instead of prompting
        #[arg(long)]
        stdin: bool,
    },
    /// Show where each provider's credential comes from
    Status,
    /// Remove stored credentials for a provider
    Unset {
        /// Provider: deepseek or openai
        provider: String,
    },
}

#[derive(Debug, Subcommand)]
enum ProviderCommand {
    /// List built-in presets and whether their API key is present
    List,
    /// Stream one prompt through a provider
    Test {
        /// Preset: deepseek or openai
        #[arg(long, default_value = "deepseek")]
        preset: String,
        /// Override the preset model
        #[arg(long)]
        model: Option<String>,
        /// Prompt for the test request
        #[arg(long)]
        prompt: Option<String>,
        /// Disable DeepSeek thinking mode
        #[arg(long)]
        no_thinking: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Session { command }) => run_session(command),
        Some(Command::Replay { path }) => run_replay(path),
        Some(Command::Fork {
            path,
            at,
            session_dir,
        }) => run_fork(path, at, session_dir),
        Some(Command::Worktree { command }) => run_worktree(command),
        Some(Command::Provider { command }) => run_provider(command).await,
        Some(Command::Gc {
            apply,
            prune_sessions,
            yes,
        }) => run_gc(apply, prune_sessions, yes),
        Some(Command::Auth { command }) => run_auth(command),
        Some(Command::Exec {
            prompt,
            preset,
            model,
            max_steps,
            context_budget,
            keep_recent_turns,
            no_thinking,
            cwd,
            session_dir,
            resume,
            record,
            replay,
            sandbox,
            approval,
            network,
            json,
            allow_subagent_inheritance,
            tools,
            goal,
            max_goal_turns,
        }) => {
            let code = run_exec(
                prompt,
                preset,
                model,
                max_steps,
                context_budget,
                keep_recent_turns,
                no_thinking,
                cwd,
                session_dir,
                resume,
                record,
                replay,
                sandbox,
                approval,
                network,
                json,
                allow_subagent_inheritance,
                tools,
                goal,
                max_goal_turns,
            )
            .await?;
            if code != 0 {
                std::process::exit(code);
            }
            Ok(())
        }
        Some(Command::Tui {
            preset,
            model,
            context_budget,
            max_steps,
            keep_recent_turns,
            no_thinking,
            cwd,
            session_dir,
            resume,
            sandbox,
            approval,
            network,
            replay,
            plan,
            allow_subagent_inheritance,
        }) => {
            run_tui(
                preset,
                model,
                context_budget,
                max_steps,
                keep_recent_turns,
                no_thinking,
                cwd,
                session_dir,
                resume,
                sandbox,
                approval,
                network,
                replay,
                plan,
                allow_subagent_inheritance,
            )
            .await
        }
        None => {
            run_tui(
                "deepseek".into(),
                None,
                64_000,
                0,
                3,
                false,
                None,
                None,
                None,
                "workspace-write".into(),
                "on-failure".into(),
                "off".into(),
                None,
                false,
                false,
            )
            .await
        }
    }
}

fn run_worktree(command: WorktreeCommand) -> Result<()> {
    match command {
        WorktreeCommand::Apply { patch, check } => {
            let cwd = std::env::current_dir()?;
            let mut command = std::process::Command::new("git");
            command
                .arg("-C")
                .arg(&cwd)
                .args(["apply", "--check"])
                .arg(&patch);
            let status = command.status()?;
            if !status.success() {
                bail!("patch does not apply cleanly: {}", patch.display());
            }
            if check {
                println!("patch applies cleanly: {}", patch.display());
                return Ok(());
            }
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(&cwd)
                .args(["apply"])
                .arg(&patch)
                .status()?;
            if !status.success() {
                bail!("git apply failed: {}", patch.display());
            }
            println!("applied {}", patch.display());
            Ok(())
        }
    }
}

fn run_gc(apply: bool, prune_sessions: bool, yes: bool) -> Result<()> {
    if prune_sessions && !yes {
        bail!("--prune-sessions requires --yes");
    }
    let root = workspace::global_root()?;
    let projects = root.join("projects");
    if !projects.is_dir() {
        println!("workspace {} · empty", root.display());
        return Ok(());
    }
    let mut total = 0_u64;
    let mut entries = std::fs::read_dir(&projects)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    entries.sort();
    for project in entries {
        let sizes = [
            ("sessions", project.join("sessions")),
            ("proposals", project.join("proposals")),
            ("backups", project.join("backups")),
            ("cache", project.join("cache")),
            ("worktrees", project.join("worktrees")),
        ];
        let mut project_total = 0_u64;
        for (name, path) in &sizes {
            let size = directory_size(path);
            project_total += size;
            if size > 0 {
                println!(
                    "{:<10} {:>10}  {}",
                    name,
                    human_bytes(size),
                    project.display()
                );
            }
        }
        total += project_total;
        println!(
            "{:<10} {:>10}  {}",
            "total",
            human_bytes(project_total),
            project.display()
        );
        if apply {
            removed_old_files(&project.join("backups"), 5);
            removed_old_files(&project.join("proposals"), 10);
            let _ = std::fs::remove_dir_all(project.join("cache"));
            println!("  applied derived cleanup for {}", project.display());
        }
        if prune_sessions && yes {
            removed_old_sessions(&project.join("sessions"), 30, 50);
        }
    }
    println!("workspace {} · {}", root.display(), human_bytes(total));
    if !apply && !prune_sessions {
        println!("dry-run only; use --apply for derived cleanup or --prune-sessions --yes");
    }
    Ok(())
}

fn directory_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| {
            entry
                .metadata()
                .map(|metadata| {
                    if metadata.is_dir() {
                        directory_size(&entry.path())
                    } else {
                        metadata.len()
                    }
                })
                .unwrap_or(0)
        })
        .sum()
}

fn human_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn removed_old_files(directory: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut files = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    for (_, path) in files.into_iter().skip(keep) {
        let _ = std::fs::remove_file(path);
    }
}

fn removed_old_sessions(directory: &Path, days: u64, keep: usize) {
    let cutoff =
        std::time::SystemTime::now().checked_sub(std::time::Duration::from_secs(days * 86_400));
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut files = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    for (index, (modified, path)) in files.into_iter().enumerate() {
        if index >= keep || cutoff.is_some_and(|cutoff| modified < cutoff) {
            let _ = std::fs::remove_file(path);
        }
    }
}

async fn mount_extensions(
    workspace: &workspace::Workspace,
    session: &mut SessionLog,
) -> Result<Option<Arc<ExtensionHost>>> {
    let reserved = Toolbox::specs()
        .into_iter()
        .map(|tool| tool.function.name)
        .collect::<Vec<_>>();
    let report = discover_in(
        &workspace.extensions,
        workspace.extension_workspace(),
        &reserved,
    )
    .await;
    for error in &report.errors {
        eprintln!("warning: extension: {error}");
        session.append(&Event::ExtensionError {
            name: "mount".into(),
            message: error.clone(),
            ts: now_ms(),
        })?;
    }
    if report.host.is_empty() {
        return Ok(None);
    }
    for mount in report.host.mounts() {
        eprintln!(
            "extension: {} {} · {} tool(s)",
            mount.name,
            mount.version,
            mount.tools.len()
        );
        if !mount.capabilities.is_empty() {
            eprintln!(
                "extension: {} capabilities={} workers={} digest={}",
                mount.name,
                mount.capabilities.join(","),
                mount.workers,
                mount.digest
            );
        }
        session.append(&Event::ExtensionMount {
            name: mount.name.clone(),
            version: mount.version.clone(),
            tools: mount.tools.clone(),
            capabilities: mount.capabilities.clone(),
            digest: mount.digest.clone(),
            workers: mount.workers as u64,
            ts: now_ms(),
        })?;
    }
    Ok(Some(Arc::new(report.host)))
}

async fn unmount_extensions(
    host: &Option<Arc<ExtensionHost>>,
    session: &mut SessionLog,
) -> Result<()> {
    if let Some(host) = host {
        for mount in host.mounts() {
            session.append(&Event::ExtensionUnmount {
                name: mount.name.clone(),
                ts: now_ms(),
            })?;
        }
        host.shutdown().await;
    }
    Ok(())
}

fn run_session(command: SessionCommand) -> Result<()> {
    match command {
        SessionCommand::New { dir } => {
            let dir = match dir {
                Some(dir) => dir,
                None => workspace::Workspace::discover(&std::env::current_dir()?)?.sessions,
            };
            let log = SessionLog::create(&dir)?;
            log.append(&Event::SessionStart {
                id: log.id().to_string(),
                cwd: std::env::current_dir()?.display().to_string(),
                model: "unset".into(),
                provider: "deepseek".into(),
                ts: now_ms(),
            })?;
            println!("{}", log.path().display());
            Ok(())
        }
        SessionCommand::Show { path } => {
            let loaded = niucode_core::load(&path)?;
            println!("session  {}", path.display());
            println!("events   {}", loaded.events.len());
            println!("warnings {}", loaded.warnings.len());
            for warning in &loaded.warnings {
                eprintln!("warning: {warning}");
            }
            for event in &loaded.events {
                let detail = match event {
                    Event::Compaction {
                        upto,
                        before_tokens,
                        after_tokens,
                        ..
                    } => format!("  upto={upto} ~{before_tokens}→~{after_tokens} tokens"),
                    Event::Instructions {
                        sources, digest, ..
                    } => {
                        format!("  {} file(s) digest={digest}", sources.len())
                    }
                    Event::TurnEnd { usage, .. } => format!(
                        "  in={} out={} cached={} cache={}%",
                        usage.input_tokens,
                        usage.output_tokens,
                        usage.cached_tokens,
                        usage.cache_hit_percent()
                    ),
                    _ => String::new(),
                };
                println!("{:>13}  {}{detail}", event.ts(), event.kind());
            }
            Ok(())
        }
        SessionCommand::List { dir } => {
            let dir = match dir {
                Some(dir) => dir,
                None => workspace::Workspace::discover(&std::env::current_dir()?)?.sessions,
            };
            for path in niucode_core::session::list(&dir)? {
                println!("{}", path.display());
            }
            Ok(())
        }
    }
}

fn run_replay(path: PathBuf) -> Result<()> {
    let report = session_ops::replay(&path)?;
    for warning in &report.warnings {
        eprintln!("warning: {warning}");
    }
    println!("session  {}", report.session.display());
    println!(
        "events   {} decoded (warnings {})",
        report.events,
        report.warnings.len()
    );
    println!("mode     offline projection · no provider calls");
    println!("turns    {}", report.turns.len());
    for turn in &report.turns {
        println!(
            "turn {:<3} {}  {:?}  messages={} ~{} tokens digest={}",
            turn.number, turn.event, turn.user, turn.messages, turn.tokens, turn.digest
        );
    }
    println!(
        "final    messages={} ~{} tokens digest={}",
        report.final_messages, report.final_tokens, report.final_digest
    );
    println!(
        "counts   compactions={} instructions={}",
        report.compactions, report.instructions
    );
    Ok(())
}

fn run_fork(path: PathBuf, at: String, session_dir: Option<PathBuf>) -> Result<()> {
    let destination_dir = session_dir.unwrap_or_else(|| {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    });
    let report = session_ops::fork(&path, &at, &destination_dir)?;
    for warning in &report.warnings {
        eprintln!("warning: {warning}");
    }
    println!("parent  {}", report.parent.display());
    println!("event   {}", report.at);
    println!("copied  {} events", report.copied);
    println!("fork    {}", report.destination.display());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_tui(
    preset: String,
    model: Option<String>,
    context_budget: usize,
    max_steps: usize,
    keep_recent_turns: usize,
    no_thinking: bool,
    cwd: Option<PathBuf>,
    session_dir: Option<PathBuf>,
    resume: Option<PathBuf>,
    sandbox: String,
    approval: String,
    network: String,
    replay: Option<PathBuf>,
    plan: bool,
    allow_subagent_inheritance: bool,
) -> Result<()> {
    if context_budget != 0 && context_budget < 100 {
        bail!("--context-budget must be 0 (auto) or at least 100 tokens");
    }
    if keep_recent_turns == 0 {
        bail!("--keep-recent-turns must be at least 1");
    }
    let mut approval_mode = parse_approval(&approval)?;
    let policy = SandboxPolicy::new(parse_sandbox(&sandbox)?, parse_network(&network)?);
    let mut backend = policy.backend();
    let mut allow_unsandboxed = false;
    if backend == "unavailable" {
        match approval_mode {
            ApprovalMode::Never => bail!(
                "sandbox backend bwrap is unavailable; install bubblewrap or use --approval ask"
            ),
            ApprovalMode::OnFailure => {
                eprintln!(
                    "warning: sandbox backend unavailable; falling back to per-action approval (network policy is not enforced)"
                );
                approval_mode = ApprovalMode::Ask;
                allow_unsandboxed = true;
                backend = "approval-fallback";
            }
            ApprovalMode::Ask => {
                eprintln!(
                    "warning: sandbox backend unavailable; using per-action approval (network policy is not enforced)"
                );
                allow_unsandboxed = true;
                backend = "approval-fallback";
            }
        }
    }

    let global_config = workspace::load_config()?;
    let preset = if preset == "deepseek" {
        global_config.defaults.provider.clone().unwrap_or(preset)
    } else {
        preset
    };
    let model = model.or(global_config.defaults.model.clone());
    let context_budget = if context_budget == 0 {
        global_config.defaults.context_budget.unwrap_or(0)
    } else {
        context_budget
    };
    let preset = parse_provider(&preset)?;
    let (provider, model, provider_factory) = if let Some(path) = &replay {
        let provider = ReplayProvider::load(path)?;
        let model = model.unwrap_or_else(|| "replay".into());
        (Arc::new(provider) as Arc<dyn Provider>, model, None)
    } else {
        let factory = ProviderFactory::new(no_thinking, None)?;
        let mut provider_id = preset.as_str().to_string();
        let mut model = model.unwrap_or_else(|| {
            ProviderFactory::default_model(&provider_id).unwrap_or_else(|_| "deepseek-flash".into())
        });
        if let Ok(selection) = niucode_provider::ModelSelection::parse(&model, &provider_id) {
            provider_id = selection.provider;
            model = selection.model;
        }
        let provider = factory.build(&provider_id, &model)?;
        (provider, model, Some(factory))
    };

    let context_budget_auto = context_budget == 0;
    let context_budget = if context_budget_auto {
        niucode_provider::derived_context_budget(provider.name(), &model)
    } else {
        context_budget
    };

    let resumed = match &resume {
        Some(path) => {
            let loaded = niucode_core::load(path)?;
            for warning in &loaded.warnings {
                eprintln!("warning: {warning}");
            }
            let session_cwd = loaded.events.iter().find_map(|event| match event {
                Event::SessionStart { cwd, .. } => Some(cwd.clone()),
                _ => None,
            });
            let history = project_history(&loaded.events)?;
            Some((path.clone(), history, session_cwd))
        }
        None => None,
    };

    let working_dir = match cwd {
        Some(path) => path
            .canonicalize()
            .with_context(|| format!("resolve --cwd {}", path.display()))?,
        None => match resumed.as_ref().and_then(|(_, _, cwd)| cwd.clone()) {
            Some(path) => PathBuf::from(path)
                .canonicalize()
                .context("resolve resumed session cwd")?,
            None => std::env::current_dir()?.canonicalize()?,
        },
    };

    let workspace = workspace::Workspace::discover(&working_dir)?;
    let (mut session, history) = if let Some((path, history, _)) = resumed {
        (SessionLog::open(&path)?, history)
    } else {
        let session_dir = session_dir.unwrap_or_else(|| workspace.sessions.clone());
        let session = SessionLog::create(&session_dir)?;
        session.append(&Event::SessionStart {
            id: session.id().to_string(),
            cwd: working_dir.display().to_string(),
            model: model.clone(),
            provider: provider.name().to_string(),
            ts: now_ms(),
        })?;
        (session, Vec::new())
    };

    session.append(&Event::Policy {
        sandbox: policy.mode.as_str().into(),
        approval: approval_mode.as_str().into(),
        network: policy.network.as_str().into(),
        backend: backend.into(),
        ts: now_ms(),
    })?;
    if plan {
        session.append(&Event::PlanMode {
            active: true,
            ts: now_ms(),
        })?;
    }

    let mut agent_config = AgentConfig::new(model.clone(), working_dir.clone());
    agent_config.provider = provider.name().to_string();
    agent_config.max_steps = max_steps;
    agent_config.sandbox = policy;
    agent_config.approval = approval_mode;
    agent_config.allow_unsandboxed = allow_unsandboxed;
    agent_config.context.budget_tokens = context_budget;
    agent_config.context_budget_auto = context_budget_auto;
    agent_config.context.keep_recent_turns = keep_recent_turns;
    agent_config.write_lock = Some(Arc::new(tokio::sync::Mutex::new(())));
    agent_config.worktree_root = Some(workspace.worktrees.clone());
    agent_config.allow_subagent_inheritance = allow_subagent_inheritance;
    let extension_host = mount_extensions(&workspace, &mut session).await?;
    agent_config.extension_host = extension_host.clone();

    let theme = Theme::detect_stdout();
    let width = crossterm::terminal::size()
        .map_or(80, |s| s.0 as usize)
        .saturating_sub(1)
        .max(1);
    let project = working_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    for line in presentation::startup(
        &project,
        &model,
        policy.mode.as_str(),
        policy.network.as_str(),
        approval_mode.as_str(),
        width,
    ) {
        println!("{}", theme.accent(line));
    }
    let _ = std::io::stdout().flush();

    let result = tui::run(
        provider,
        provider_factory.as_ref(),
        &mut session,
        &agent_config,
        history,
    )
    .await;
    unmount_extensions(&extension_host, &mut session).await?;
    result
}

#[allow(clippy::too_many_arguments)]
async fn run_actor_once(
    provider: &dyn Provider,
    session: &mut SessionLog,
    config: &AgentConfig,
    prompt: &str,
    history: Vec<ChatMessage>,
    approver: &mut dyn FnMut(&ApprovalRequest) -> bool,
    observer: &mut dyn FnMut(AgentEvent),
) -> Option<Result<niucode_agent::RunOutcome>> {
    let run = run_with_history(
        provider, session, config, prompt, history, approver, observer,
    );
    tokio::pin!(run);
    tokio::select! {
        result = &mut run => Some(result),
        _ = tokio::signal::ctrl_c() => None,
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_exec(
    prompt: String,
    preset: String,
    model: Option<String>,
    max_steps: usize,
    context_budget: usize,
    keep_recent_turns: usize,
    no_thinking: bool,
    cwd: Option<PathBuf>,
    session_dir: Option<PathBuf>,
    resume: Option<PathBuf>,
    record: Option<PathBuf>,
    replay: Option<PathBuf>,
    sandbox: String,
    approval: String,
    network: String,
    json: bool,
    allow_subagent_inheritance: bool,
    tools: Option<Vec<String>>,
    goal: Option<String>,
    max_goal_turns: usize,
) -> Result<i32> {
    if record.is_some() && replay.is_some() {
        bail!("--record and --replay are mutually exclusive");
    }
    if context_budget != 0 && context_budget < 100 {
        bail!("--context-budget must be 0 (auto) or at least 100 tokens");
    }
    if keep_recent_turns == 0 {
        bail!("--keep-recent-turns must be at least 1");
    }
    if max_goal_turns == 0 {
        bail!("--max-goal-turns must be at least 1");
    }

    let mut approval_mode = parse_approval(&approval)?;
    let policy = SandboxPolicy::new(parse_sandbox(&sandbox)?, parse_network(&network)?);
    let mut backend = policy.backend();
    let mut allow_unsandboxed = false;
    if backend == "unavailable" {
        match approval_mode {
            ApprovalMode::Never => bail!(
                "sandbox backend bwrap is unavailable; install bubblewrap or use --approval ask"
            ),
            ApprovalMode::OnFailure => {
                eprintln!(
                    "warning: sandbox backend unavailable; falling back to per-action approval (network policy is not enforced)"
                );
                approval_mode = ApprovalMode::Ask;
                allow_unsandboxed = true;
                backend = "approval-fallback";
            }
            ApprovalMode::Ask => {
                eprintln!(
                    "warning: sandbox backend unavailable; using per-action approval (network policy is not enforced)"
                );
                allow_unsandboxed = true;
                backend = "approval-fallback";
            }
        }
    }

    if json && approval_mode == ApprovalMode::Ask {
        bail!("--json cannot prompt for approval; use --approval never or a working sandbox");
    }

    let global_config = workspace::load_config()?;
    let preset = if preset == "deepseek" {
        global_config.defaults.provider.clone().unwrap_or(preset)
    } else {
        preset
    };
    let model = model.or(global_config.defaults.model.clone());
    let context_budget = if context_budget == 0 {
        global_config.defaults.context_budget.unwrap_or(0)
    } else {
        context_budget
    };
    let preset = parse_provider(&preset)?;
    let (provider, model, auth_source) = if let Some(path) = &replay {
        let provider = ReplayProvider::load(path)?;
        let model = model.unwrap_or_else(|| "replay".into());
        (
            Arc::new(provider) as Arc<dyn Provider>,
            model,
            "replay".into(),
        )
    } else {
        let factory = ProviderFactory::new(no_thinking, record.clone())?;
        let mut provider_id = preset.as_str().to_string();
        let mut model = model.unwrap_or_else(|| {
            ProviderFactory::default_model(&provider_id).unwrap_or_else(|_| "deepseek-flash".into())
        });
        if let Ok(selection) = niucode_provider::ModelSelection::parse(&model, &provider_id) {
            provider_id = selection.provider;
            model = selection.model;
        }
        if let Some(path) = &record {
            eprintln!("recording fixture: {}", path.display());
        }
        let provider = factory.build(&provider_id, &model)?;
        let auth_source = factory
            .auth_source(&provider_id)?
            .unwrap_or_else(|| "unknown".into());
        (provider, model, auth_source)
    };

    let context_budget_auto = context_budget == 0;
    let context_budget = if context_budget_auto {
        niucode_provider::derived_context_budget(provider.name(), &model)
    } else {
        context_budget
    };
    let resumed = match &resume {
        Some(path) => {
            let loaded = niucode_core::load(path)?;
            for warning in &loaded.warnings {
                eprintln!("warning: {warning}");
            }
            let session_cwd = loaded.events.iter().find_map(|event| match event {
                Event::SessionStart { cwd, .. } => Some(cwd.clone()),
                _ => None,
            });
            let history = project_history(&loaded.events)?;
            Some((path.clone(), history, session_cwd))
        }
        None => None,
    };

    let working_dir = match cwd {
        Some(path) => path
            .canonicalize()
            .with_context(|| format!("resolve --cwd {}", path.display()))?,
        None => match resumed.as_ref().and_then(|(_, _, cwd)| cwd.clone()) {
            Some(path) => PathBuf::from(path)
                .canonicalize()
                .context("resolve resumed session cwd")?,
            None => std::env::current_dir()?.canonicalize()?,
        },
    };

    let workspace = workspace::Workspace::discover(&working_dir)?;
    let (mut session, history) = if let Some((path, history, _)) = resumed {
        (SessionLog::open(&path)?, history)
    } else {
        let session_dir = session_dir.unwrap_or_else(|| workspace.sessions.clone());
        let session = SessionLog::create(&session_dir)?;
        session.append(&Event::SessionStart {
            id: session.id().to_string(),
            cwd: working_dir.display().to_string(),
            model: model.clone(),
            provider: provider.name().to_string(),
            ts: now_ms(),
        })?;
        (session, Vec::new())
    };

    session.append(&Event::Policy {
        sandbox: policy.mode.as_str().into(),
        approval: approval_mode.as_str().into(),
        network: policy.network.as_str().into(),
        backend: backend.into(),
        ts: now_ms(),
    })?;

    if !json {
        eprintln!(
            "niucode exec · {} · model={model} · auth={auth_source} · cwd={}",
            provider.name(),
            working_dir.display()
        );
        eprintln!(
            "policy: sandbox={} network={} approval={} backend={backend}",
            policy.mode.as_str(),
            policy.network.as_str(),
            approval_mode.as_str()
        );
        eprintln!("context: budget={context_budget} trigger=75% keep_recent={keep_recent_turns}");
        if let Some(path) = &replay {
            eprintln!("replay: {}", path.display());
        }
        if !history.is_empty() {
            eprintln!("resumed: {} messages", history.len());
        }
        eprintln!("session: {}", session.path().display());
    }

    let mut agent_config = AgentConfig::new(model, working_dir);
    agent_config.provider = provider.name().to_string();
    agent_config.max_steps = max_steps;
    agent_config.sandbox = policy;
    agent_config.approval = approval_mode;
    agent_config.allow_unsandboxed = allow_unsandboxed;
    agent_config.context.budget_tokens = context_budget;
    agent_config.context_budget_auto = context_budget_auto;
    agent_config.context.keep_recent_turns = keep_recent_turns;
    agent_config.write_lock = Some(Arc::new(tokio::sync::Mutex::new(())));
    agent_config.worktree_root = Some(workspace.worktrees.clone());
    agent_config.allow_subagent_inheritance = allow_subagent_inheritance;
    let extension_host = mount_extensions(&workspace, &mut session).await?;
    agent_config.extension_host = extension_host.clone();
    agent_config.allowed_tools = tools;
    if let Ok(registry) = skills::Registry::discover(&workspace) {
        let (block, roots) = registry.activate(&prompt);
        if !block.is_empty() {
            agent_config.system_prompt.push_str(&block);
            agent_config.read_roots.extend(roots);
        }
    }
    if let Some(condition) = &goal {
        session.append(&Event::GoalSet {
            text: condition.clone(),
            ts: now_ms(),
        })?;
        agent_config.goal = Some(condition.clone());
    }

    if json {
        emit_json(json!({
            "type": "start",
            "protocol": 1,
            "session": session.path().display().to_string(),
            "model": agent_config.model,
            "provider": provider.name(),
            "cwd": agent_config.working_dir.display().to_string(),
            "sandbox": policy.mode.as_str(),
            "network": policy.network.as_str(),
            "approval": approval_mode.as_str(),
            "backend": backend,
        }));
    }

    let mut approver = |request: &ApprovalRequest| -> bool {
        if json {
            emit_json(json!({
                "type": "warning",
                "message": format!("approval requested in json mode; denied: {}", request.summary),
            }));
            return false;
        }
        eprintln!(
            "\napproval required: {}",
            text_layout::clean_text(&request.summary)
        );
        if let Some(preview) = &request.preview {
            eprint!("{}", diff::render_plain(preview, 100));
        }
        eprint!("allow? [y/N] ");
        let _ = std::io::stderr().flush();
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_err() {
            return false;
        }
        matches!(line.trim(), "y" | "Y" | "yes" | "YES")
    };

    let mut line_open = false;
    let mut observer = |event: AgentEvent| {
        if json {
            emit_json(event_json(&event));
            return;
        }
        match event {
            AgentEvent::Phase(_) => {}
            AgentEvent::TextDelta(delta) => {
                print!("{delta}");
                let _ = std::io::stdout().flush();
                line_open = !delta.ends_with('\n');
            }
            AgentEvent::ReasoningDelta(_) => {}
            AgentEvent::ToolCall { name, input } => {
                if line_open {
                    println!();
                    line_open = false;
                }
                eprintln!("→ {name} {}", compact_json(&input));
            }
            AgentEvent::ToolResult {
                name, ok, summary, ..
            } => {
                eprintln!("← {name} {} {}", if ok { "ok" } else { "error" }, summary);
            }
            AgentEvent::TurnEnd { usage } => {
                if line_open {
                    println!();
                    line_open = false;
                }
                eprintln!(
                    "turn usage: in={} out={} cached={} cache={}%",
                    usage.input_tokens,
                    usage.output_tokens,
                    usage.cached_tokens,
                    usage.cache_hit_percent()
                );
            }
            AgentEvent::Compaction {
                replaced_events,
                before_tokens,
                after_tokens,
            } => {
                if line_open {
                    println!();
                    line_open = false;
                }
                eprintln!(
                    "compacted: {replaced_events} events · ~{before_tokens} → ~{after_tokens} tokens"
                );
            }
            AgentEvent::Warning(message) => {
                if line_open {
                    println!();
                    line_open = false;
                }
                eprintln!("warning: {}", text_layout::clean_text(&message));
            }
        }
    };
    let result = run_actor_once(
        provider.as_ref(),
        &mut session,
        &agent_config,
        &prompt,
        history,
        &mut approver,
        &mut observer,
    )
    .await;

    if result.is_none() {
        unmount_extensions(&extension_host, &mut session).await?;
        let _ = session.append(&Event::SessionEnd { ts: now_ms() });
        if json {
            emit_json(json!({"type": "cancelled"}));
        } else {
            eprintln!("\ncancelled");
        }
        return Ok(130);
    }

    let result = result.expect("checked above");
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(error) => {
            unmount_extensions(&extension_host, &mut session).await?;
            if json {
                emit_json(json!({"type": "error", "message": format!("{error:#}")}));
            }
            eprintln!("session: {}", session.path().display());
            return Err(error);
        }
    };
    let mut outcome = outcome;
    if let Some(condition) = goal.as_deref() {
        let mut rounds = 1_usize;
        loop {
            let verdict = match goal::evaluate(
                provider.as_ref(),
                &agent_config.model,
                condition,
                &outcome.final_text,
            )
            .await
            {
                Ok(verdict) => verdict,
                Err(error) => {
                    unmount_extensions(&extension_host, &mut session).await?;
                    let _ = session.append(&Event::SessionEnd { ts: now_ms() });
                    if json {
                        emit_json(
                            json!({"type": "error", "message": format!("goal evaluator: {error:#}")}),
                        );
                    }
                    return Err(error);
                }
            };
            session.append(&Event::GoalVerdict {
                status: verdict.status.as_str().into(),
                reason: verdict.reason.clone(),
                ts: now_ms(),
            })?;
            if json {
                emit_json(json!({
                    "type": "goal_verdict",
                    "status": verdict.status.as_str(),
                    "reason": verdict.reason,
                }));
            } else {
                eprintln!(
                    "goal: {} · {}",
                    verdict.status.as_str(),
                    text_layout::clean_text(&verdict.reason)
                );
            }
            match verdict.status {
                goal::GoalStatus::Met => {
                    session.append(&Event::GoalCleared { ts: now_ms() })?;
                    break;
                }
                goal::GoalStatus::Impossible => {
                    unmount_extensions(&extension_host, &mut session).await?;
                    let _ = session.append(&Event::SessionEnd { ts: now_ms() });
                    if json {
                        emit_json(json!({"type": "goal_done", "status": "impossible"}));
                    }
                    return Ok(3);
                }
                goal::GoalStatus::NotMet => {
                    if rounds >= max_goal_turns {
                        unmount_extensions(&extension_host, &mut session).await?;
                        let _ = session.append(&Event::SessionEnd { ts: now_ms() });
                        if json {
                            emit_json(json!({"type": "goal_done", "status": "budget"}));
                        }
                        return Ok(2);
                    }
                    rounds += 1;
                    let loaded = niucode_core::load(session.path())?;
                    let history = project_history(&loaded.events)?;
                    let Some(actor) = run_actor_once(
                        provider.as_ref(),
                        &mut session,
                        &agent_config,
                        &goal::continue_prompt(condition),
                        history,
                        &mut approver,
                        &mut observer,
                    )
                    .await
                    else {
                        unmount_extensions(&extension_host, &mut session).await?;
                        let _ = session.append(&Event::SessionEnd { ts: now_ms() });
                        if json {
                            emit_json(json!({"type": "cancelled"}));
                        }
                        return Ok(130);
                    };
                    outcome = match actor {
                        Ok(outcome) => outcome,
                        Err(error) => {
                            unmount_extensions(&extension_host, &mut session).await?;
                            if json {
                                emit_json(
                                    json!({"type": "error", "message": format!("{error:#}")}),
                                );
                            }
                            return Err(error);
                        }
                    };
                }
            }
        }
    }
    if let Some(path) = &record {
        eprintln!("fixture: {}", path.display());
    }
    unmount_extensions(&extension_host, &mut session).await?;

    if json {
        let mut done = json!({
            "type": "done",
            "turns": outcome.turns,
            "usage": usage_json(&outcome.usage),
            "session": session.path().display().to_string(),
        });
        if goal.is_some() {
            done["goal"] = json!("met");
        }
        emit_json(done);
    } else {
        if !outcome.final_text.ends_with('\n') {
            println!();
        }
        eprintln!(
            "done: {} turns, input={} output={} cached={} cache={}% · {}",
            outcome.turns,
            outcome.usage.input_tokens,
            outcome.usage.output_tokens,
            outcome.usage.cached_tokens,
            outcome.usage.cache_hit_percent(),
            session.path().display()
        );
    }
    Ok(0)
}

fn emit_json(value: serde_json::Value) {
    match serde_json::to_string(&value) {
        Ok(line) => {
            println!("{line}");
            let _ = std::io::stdout().flush();
        }
        Err(error) => eprintln!("json encode error: {error}"),
    }
}

fn usage_json(usage: &niucode_core::Usage) -> serde_json::Value {
    json!({
        "input_tokens": usage.input_tokens,
        "output_tokens": usage.output_tokens,
        "cached_tokens": usage.cached_tokens,
        "reasoning_tokens": usage.reasoning_tokens,
        "cache_hit_percent": usage.cache_hit_percent(),
    })
}

fn event_json(event: &AgentEvent) -> serde_json::Value {
    match event {
        AgentEvent::Phase(phase) => json!({"type": "phase", "phase": phase.as_str()}),
        AgentEvent::TextDelta(text) => json!({"type": "text_delta", "text": text}),
        AgentEvent::ReasoningDelta(text) => json!({"type": "reasoning_delta", "text": text}),
        AgentEvent::ToolCall { name, input } => {
            json!({"type": "tool_call", "name": name, "input": input})
        }
        AgentEvent::ToolResult {
            name, ok, summary, ..
        } => json!({
            "type": "tool_result",
            "name": name,
            "ok": ok,
            "summary": summary,
        }),
        AgentEvent::TurnEnd { usage } => {
            json!({"type": "turn_end", "usage": usage_json(usage)})
        }
        AgentEvent::Compaction {
            replaced_events,
            before_tokens,
            after_tokens,
        } => json!({
            "type": "compaction",
            "replaced_events": replaced_events,
            "before_tokens": before_tokens,
            "after_tokens": after_tokens,
        }),
        AgentEvent::Warning(message) => json!({"type": "warning", "message": message}),
    }
}

fn parse_sandbox(value: &str) -> Result<SandboxMode> {
    match value {
        "read-only" => Ok(SandboxMode::ReadOnly),
        "workspace-write" => Ok(SandboxMode::WorkspaceWrite),
        "full-access" => Ok(SandboxMode::FullAccess),
        other => {
            bail!("invalid --sandbox {other:?}; use read-only, workspace-write, or full-access")
        }
    }
}

fn parse_network(value: &str) -> Result<NetworkMode> {
    match value {
        "on" => Ok(NetworkMode::On),
        "off" => Ok(NetworkMode::Off),
        other => bail!("invalid --network {other:?}; use on or off"),
    }
}

fn parse_approval(value: &str) -> Result<ApprovalMode> {
    match value {
        "ask" => Ok(ApprovalMode::Ask),
        "on-failure" => Ok(ApprovalMode::OnFailure),
        "never" => Ok(ApprovalMode::Never),
        other => bail!("invalid --approval {other:?}; use ask, on-failure, or never"),
    }
}

fn compact_json(value: &serde_json::Value) -> String {
    let rendered = serde_json::to_string(value).unwrap_or_default();
    rendered.chars().take(200).collect()
}

async fn run_provider(command: ProviderCommand) -> Result<()> {
    match command {
        ProviderCommand::List => list_providers(),
        ProviderCommand::Test {
            preset,
            model,
            prompt,
            no_thinking,
        } => {
            let preset = parse_provider(&preset)?;
            let store = AuthStore::new()?;
            let credential = match store.resolve(preset.as_str())? {
                Some(credential) => credential,
                None => bail!(
                    "no credential for {}; run `niucode auth set {}` or set {}",
                    preset.as_str(),
                    preset.as_str(),
                    preset.api_key_env()
                ),
            };

            let mut config = OpenAiConfig::preset_config(preset)?;
            config.api_key = credential.key.expose().to_string();
            if let Some(model) = model {
                config.model = model;
            }
            if no_thinking && preset == Preset::DeepSeek {
                config.thinking = Some(ThinkingMode::Disabled);
            }

            let provider = OpenAiProvider::for_preset(preset, config)?;
            let model = provider.model().to_string();
            let prompt = prompt.unwrap_or_else(|| "Reply with exactly: niucode provider ok".into());
            let request = CompletionRequest::new(&model).with_message(ChatMessage::user(prompt));

            eprintln!(
                "provider: {} · model: {model} · auth: {}",
                provider.name(),
                credential.source
            );
            let mut stream = provider.stream(request).await?;
            let mut text = String::new();
            let mut reasoning_chars = 0_usize;
            let mut tool_calls = 0_usize;
            let mut usage = None;
            let mut finish_reason = None;

            while let Some(event) = stream.next().await {
                match event? {
                    StreamEvent::TextDelta(delta) => {
                        print!("{delta}");
                        std::io::stdout().flush()?;
                        text.push_str(&delta);
                    }
                    StreamEvent::ReasoningDelta(delta) => {
                        reasoning_chars += delta.chars().count();
                    }
                    StreamEvent::ToolCallDelta(call) => {
                        if let Some(name) = call.name {
                            tool_calls += 1;
                            eprintln!("\n[tool] {name}");
                        }
                    }
                    StreamEvent::Usage(value) => usage = Some(value),
                    StreamEvent::Done {
                        finish_reason: reason,
                    } => finish_reason = reason,
                }
            }

            if !text.ends_with('\n') {
                println!();
            }
            if let Some(usage) = usage {
                eprintln!(
                    "usage: input={} output={} cached={} reasoning={}",
                    usage.input_tokens,
                    usage.output_tokens,
                    usage.cached_tokens,
                    usage.reasoning_tokens
                );
            }
            if reasoning_chars > 0 || tool_calls > 0 || finish_reason.is_some() {
                eprintln!(
                    "stream: reasoning_chars={reasoning_chars} tool_calls={tool_calls} finish={}",
                    finish_reason.as_deref().unwrap_or("none")
                );
            }
            if text.is_empty() && tool_calls == 0 {
                bail!("provider returned no text and no tool calls");
            }
            Ok(())
        }
    }
}

fn run_auth(command: AuthCommand) -> Result<()> {
    match command {
        AuthCommand::Set { provider, stdin } => {
            let preset = parse_provider(&provider)?;
            let store = AuthStore::new()?;

            if !stdin {
                eprintln!("Create a key at {}", key_url(preset));
            }

            let key = if stdin {
                let mut line = String::new();
                std::io::stdin().lock().read_line(&mut line)?;
                line
            } else {
                rpassword::prompt_password(format!("Paste {} API key (hidden): ", preset.as_str()))?
            };

            let storage = store.set(preset.as_str(), key.trim())?;
            println!("stored {} in {}", preset.as_str(), storage);
            println!("credentials {}", store.credentials_path().display());
            Ok(())
        }
        AuthCommand::Status => {
            let store = AuthStore::new()?;
            println!(
                "{:<8} {:<10} {:<8} {:<8} file",
                "provider", "effective", "env", "keyring"
            );
            for preset in Preset::ALL {
                let status = store.status(preset.as_str())?;
                let effective = status
                    .effective
                    .map(|source| source.as_str())
                    .unwrap_or("missing");
                let file = if !status.file {
                    "missing"
                } else if status.file_private {
                    "set"
                } else {
                    "insecure"
                };
                println!(
                    "{:<8} {:<10} {:<8} {:<8} {}",
                    preset.as_str(),
                    effective,
                    yes_no(status.env),
                    yes_no(status.keyring),
                    file
                );
                if status.file && !status.file_private {
                    eprintln!(
                        "warning: {} is readable by other users; run chmod 600",
                        store.credentials_path().display()
                    );
                }
            }
            println!();
            println!("credentials {}", store.credentials_path().display());
            Ok(())
        }
        AuthCommand::Unset { provider } => {
            let preset = parse_provider(&provider)?;
            let store = AuthStore::new()?;
            if store.unset(preset.as_str())? {
                println!("removed {} credentials", preset.as_str());
            } else {
                println!("no stored credentials for {}", preset.as_str());
            }
            Ok(())
        }
    }
}

fn list_providers() -> Result<()> {
    let store = AuthStore::new()?;
    for preset in Preset::ALL {
        let env = preset.api_key_env();
        let status = store.status(preset.as_str())?;
        let auth = status
            .effective
            .map(|source| source.as_str())
            .unwrap_or("missing");
        let base = preset_env(preset, true).unwrap_or_else(|| preset.default_base_url().into());
        let model = preset_env(preset, false).unwrap_or_else(|| preset.default_model().into());

        println!(
            "{:<8} base={:<32} model={:<18} auth={:<8} ({env})",
            preset.as_str(),
            base,
            model,
            auth
        );
    }
    Ok(())
}

fn parse_provider(value: &str) -> Result<Preset> {
    Preset::parse(value)
        .with_context(|| format!("unknown provider {value:?}; expected deepseek or openai"))
}

fn key_url(preset: Preset) -> &'static str {
    match preset {
        Preset::DeepSeek => "https://platform.deepseek.com/api_keys",
        Preset::OpenAi => "https://platform.openai.com/api-keys",
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "set"
    } else {
        "missing"
    }
}

fn preset_env(preset: Preset, base: bool) -> Option<String> {
    let name = match (preset, base) {
        (Preset::DeepSeek, true) => "DEEPSEEK_BASE_URL",
        (Preset::DeepSeek, false) => "DEEPSEEK_MODEL",
        (Preset::OpenAi, true) => "OPENAI_BASE_URL",
        (Preset::OpenAi, false) => "OPENAI_MODEL",
    };
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use niucode_core::Usage;

    #[test]
    fn json_protocol_maps_agent_events() {
        let text = event_json(&AgentEvent::TextDelta("hi".into()));
        assert_eq!(text["type"], "text_delta");
        assert_eq!(text["text"], "hi");

        let tool = event_json(&AgentEvent::ToolResult {
            name: "read".into(),
            ok: true,
            summary: "read a.txt".into(),
            output: "full output is not part of the protocol".into(),
        });
        assert_eq!(tool["type"], "tool_result");
        assert_eq!(tool["ok"], true);
        assert_eq!(tool["summary"], "read a.txt");
        assert!(tool.get("output").is_none());

        let turn = event_json(&AgentEvent::TurnEnd {
            usage: Usage {
                input_tokens: 100,
                output_tokens: 10,
                cached_tokens: 42,
                reasoning_tokens: 0,
            },
        });
        assert_eq!(turn["type"], "turn_end");
        assert_eq!(turn["usage"]["cached_tokens"], 42);
        assert_eq!(turn["usage"]["cache_hit_percent"], 42);
    }

    #[test]
    fn json_protocol_maps_compaction_and_warnings() {
        let compaction = event_json(&AgentEvent::Compaction {
            replaced_events: 9,
            before_tokens: 2000,
            after_tokens: 300,
        });
        assert_eq!(compaction["type"], "compaction");
        assert_eq!(compaction["replaced_events"], 9);

        let warning = event_json(&AgentEvent::Warning("careful".into()));
        assert_eq!(warning["type"], "warning");
        assert_eq!(warning["message"], "careful");
    }
}
