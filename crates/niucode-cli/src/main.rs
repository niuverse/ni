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
use niucode_extension::{discover, ExtensionHost};
use niucode_provider::{
    ChatMessage, CompletionRequest, OpenAiConfig, OpenAiProvider, Preset, Provider,
    RecordingProvider, ReplayProvider, StreamEvent, ThinkingMode,
};
use niucode_tools::{NetworkMode, SandboxMode, SandboxPolicy, Toolbox};
use serde_json::json;

use crate::theme::{Theme, Tone};

mod diff;
mod editor;
mod goal;
mod review;
mod session_ops;
mod slash;
mod theme;
mod tui;

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
        /// Maximum model turns per user request
        #[arg(long, default_value_t = 24)]
        max_steps: usize,
        /// Approximate context budget in tokens before compaction
        #[arg(long, default_value_t = 64_000)]
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
    },
    /// Start the interactive linear TUI
    Tui {
        /// Provider preset: deepseek or openai
        #[arg(long, default_value = "deepseek")]
        preset: String,
        /// Override the preset model
        #[arg(long)]
        model: Option<String>,
        /// Approximate context budget in tokens before compaction
        #[arg(long, default_value_t = 64_000)]
        context_budget: usize,
        /// Maximum model turns per user request
        #[arg(long, default_value_t = 24)]
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
        Some(Command::Provider { command }) => run_provider(command).await,
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
        }) => {
            run_exec(
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
            )
            .await
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
                24,
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

async fn mount_extensions(
    workspace: &Path,
    session: &mut SessionLog,
) -> Result<Option<Arc<ExtensionHost>>> {
    let reserved = Toolbox::specs()
        .into_iter()
        .map(|tool| tool.function.name)
        .collect::<Vec<_>>();
    let report = discover(workspace, &reserved).await;
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
        session.append(&Event::ExtensionMount {
            name: mount.name.clone(),
            version: mount.version.clone(),
            tools: mount.tools.clone(),
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
            let dir = dir.unwrap_or_else(default_sessions_dir);
            let mut log = SessionLog::create(&dir)?;
            log.append(&Event::SessionStart {
                id: log.id().to_string(),
                cwd: std::env::current_dir()?.display().to_string(),
                model: "unset".into(),
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
            let dir = dir.unwrap_or_else(default_sessions_dir);
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
    if context_budget < 100 {
        bail!("--context-budget must be at least 100 tokens");
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

    let preset = parse_provider(&preset)?;
    let (provider, model, auth_source): (Arc<dyn Provider>, String, String) =
        if let Some(path) = &replay {
            let provider = ReplayProvider::load(path)?;
            let model = model.unwrap_or_else(|| "replay".into());
            (Arc::new(provider), model, "replay".into())
        } else {
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
            let model = config.model.clone();
            let provider: Arc<dyn Provider> = Arc::new(OpenAiProvider::for_preset(preset, config)?);
            (provider, model, credential.source.to_string())
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

    let (mut session, history) = if let Some((path, history, _)) = resumed {
        (SessionLog::open(&path)?, history)
    } else {
        let session_dir = session_dir.unwrap_or_else(default_sessions_dir);
        let mut session = SessionLog::create(&session_dir)?;
        session.append(&Event::SessionStart {
            id: session.id().to_string(),
            cwd: working_dir.display().to_string(),
            model: model.clone(),
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
    agent_config.max_steps = max_steps;
    agent_config.sandbox = policy;
    agent_config.approval = approval_mode;
    agent_config.allow_unsandboxed = allow_unsandboxed;
    agent_config.context.budget_tokens = context_budget;
    agent_config.context.keep_recent_turns = keep_recent_turns;
    agent_config.write_lock = Some(Arc::new(tokio::sync::Mutex::new(())));
    agent_config.allow_subagent_inheritance = allow_subagent_inheritance;
    let extension_host = mount_extensions(&working_dir, &mut session).await?;
    agent_config.extension_host = extension_host.clone();

    let theme = Theme::detect_stderr();
    eprintln!(
        "{} {}",
        theme.accent("niucode tui"),
        theme.dim(format!(
            "· {} · model={model} · auth={auth_source} · cwd={}",
            provider.name(),
            working_dir.display()
        ))
    );
    if let Some(path) = &replay {
        eprintln!("{}", theme.dim(format!("replay {}", path.display())));
    }
    eprintln!(
        "{}",
        theme.dim(format!(
            "policy sandbox={} network={} approval={} backend={backend}",
            policy.mode.as_str(),
            policy.network.as_str(),
            approval_mode.as_str()
        ))
    );
    eprintln!(
        "{}",
        theme.dim(format!(
            "context budget={context_budget} trigger=75% keep_recent={keep_recent_turns}"
        ))
    );
    eprintln!(
        "{} {}",
        theme.dim("session"),
        theme.paint(Tone::Bright, session.path().display())
    );

    let result = tui::run(provider.as_ref(), &mut session, &agent_config, history).await;
    unmount_extensions(&extension_host, &mut session).await?;
    result
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
) -> Result<()> {
    if record.is_some() && replay.is_some() {
        bail!("--record and --replay are mutually exclusive");
    }
    if context_budget < 100 {
        bail!("--context-budget must be at least 100 tokens");
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

    if json && approval_mode == ApprovalMode::Ask {
        bail!("--json cannot prompt for approval; use --approval never or a working sandbox");
    }

    let preset = parse_provider(&preset)?;
    let (provider, model, auth_source): (Arc<dyn Provider>, String, String) =
        if let Some(path) = &replay {
            let provider = ReplayProvider::load(path)?;
            let model = model.unwrap_or_else(|| "replay".into());
            (Arc::new(provider), model, "replay".into())
        } else {
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
            let model = config.model.clone();
            let inner: Arc<dyn Provider> = Arc::new(OpenAiProvider::for_preset(preset, config)?);
            let provider: Arc<dyn Provider> = if let Some(path) = &record {
                eprintln!("recording fixture: {}", path.display());
                Arc::new(RecordingProvider::new(inner, path.clone()))
            } else {
                inner
            };

            (provider, model, credential.source.to_string())
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

    let (mut session, history) = if let Some((path, history, _)) = resumed {
        (SessionLog::open(&path)?, history)
    } else {
        let session_dir = session_dir.unwrap_or_else(default_sessions_dir);
        let mut session = SessionLog::create(&session_dir)?;
        session.append(&Event::SessionStart {
            id: session.id().to_string(),
            cwd: working_dir.display().to_string(),
            model: model.clone(),
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
    agent_config.max_steps = max_steps;
    agent_config.sandbox = policy;
    agent_config.approval = approval_mode;
    agent_config.allow_unsandboxed = allow_unsandboxed;
    agent_config.context.budget_tokens = context_budget;
    agent_config.context.keep_recent_turns = keep_recent_turns;
    agent_config.write_lock = Some(Arc::new(tokio::sync::Mutex::new(())));
    agent_config.allow_subagent_inheritance = allow_subagent_inheritance;
    let extension_host = mount_extensions(&agent_config.working_dir, &mut session).await?;
    agent_config.extension_host = extension_host.clone();

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
            diff::clean_text(&request.summary)
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
    let result = {
        let mut observer = |event: AgentEvent| {
            if json {
                emit_json(event_json(&event));
                return;
            }
            match event {
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
                    eprintln!("warning: {}", diff::clean_text(&message));
                }
            }
        };
        let run = run_with_history(
            provider.as_ref(),
            &mut session,
            &agent_config,
            &prompt,
            history,
            &mut approver,
            &mut observer,
        );
        tokio::pin!(run);
        tokio::select! {
            result = &mut run => Some(result),
            _ = tokio::signal::ctrl_c() => None,
        }
    };

    if result.is_none() {
        unmount_extensions(&extension_host, &mut session).await?;
        let _ = session.append(&Event::SessionEnd { ts: now_ms() });
        if json {
            emit_json(json!({"type": "cancelled"}));
        } else {
            eprintln!("\ncancelled");
        }
        return Ok(());
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
    if let Some(path) = &record {
        eprintln!("fixture: {}", path.display());
    }
    unmount_extensions(&extension_host, &mut session).await?;

    if json {
        emit_json(json!({
            "type": "done",
            "turns": outcome.turns,
            "usage": usage_json(&outcome.usage),
            "session": session.path().display().to_string(),
        }));
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
    Ok(())
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

fn default_sessions_dir() -> PathBuf {
    if let Ok(home) = std::env::var("NIUCODE_HOME") {
        return PathBuf::from(home).join("sessions");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".niucode").join("sessions");
    }
    PathBuf::from(".niucode").join("sessions")
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
