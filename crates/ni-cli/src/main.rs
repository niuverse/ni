use std::io::{BufRead, Write};
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use futures_util::StreamExt;
use ni_agent::{run as run_agent, AgentConfig, AgentEvent};
use ni_auth::AuthStore;
use ni_core::{now_ms, Event, SessionLog};
use ni_provider::{
    ChatMessage, CompletionRequest, OpenAiConfig, OpenAiProvider, Preset, Provider, StreamEvent,
    ThinkingMode,
};

#[derive(Debug, Parser)]
#[command(name = "ni", version, about = "Minimal, observable coding agent")]
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
        /// Maximum model turns
        #[arg(long, default_value_t = 12)]
        max_steps: usize,
        /// Disable DeepSeek thinking mode
        #[arg(long)]
        no_thinking: bool,
        /// Working directory (defaults to cwd)
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Directory for the session JSONL (defaults to ~/.ni/sessions)
        #[arg(long)]
        session_dir: Option<PathBuf>,
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
        Some(Command::Provider { command }) => run_provider(command).await,
        Some(Command::Auth { command }) => run_auth(command),
        Some(Command::Exec {
            prompt,
            preset,
            model,
            max_steps,
            no_thinking,
            cwd,
            session_dir,
        }) => {
            run_exec(
                prompt,
                preset,
                model,
                max_steps,
                no_thinking,
                cwd,
                session_dir,
            )
            .await
        }
        None => {
            print_banner();
            Ok(())
        }
    }
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
            let loaded = ni_core::load(&path)?;
            println!("session  {}", path.display());
            println!("events   {}", loaded.events.len());
            println!("warnings {}", loaded.warnings.len());
            for warning in &loaded.warnings {
                eprintln!("warning: {warning}");
            }
            for event in &loaded.events {
                println!("{:>13}  {}", event.ts(), event.kind());
            }
            Ok(())
        }
        SessionCommand::List { dir } => {
            let dir = dir.unwrap_or_else(default_sessions_dir);
            for path in ni_core::session::list(&dir)? {
                println!("{}", path.display());
            }
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_exec(
    prompt: String,
    preset: String,
    model: Option<String>,
    max_steps: usize,
    no_thinking: bool,
    cwd: Option<PathBuf>,
    session_dir: Option<PathBuf>,
) -> Result<()> {
    let preset = parse_provider(&preset)?;
    let store = AuthStore::new()?;
    let credential = match store.resolve(preset.as_str())? {
        Some(credential) => credential,
        None => bail!(
            "no credential for {}; run `ni auth set {}` or set {}",
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
    let provider = OpenAiProvider::for_preset(preset, config)?;

    let working_dir = match cwd {
        Some(path) => path
            .canonicalize()
            .with_context(|| format!("resolve --cwd {}", path.display()))?,
        None => std::env::current_dir()?.canonicalize()?,
    };

    let session_dir = session_dir.unwrap_or_else(default_sessions_dir);
    let mut session = SessionLog::create(&session_dir)?;
    session.append(&Event::SessionStart {
        id: session.id().to_string(),
        cwd: working_dir.display().to_string(),
        model: model.clone(),
        ts: now_ms(),
    })?;

    eprintln!(
        "ni exec · {} · model={model} · auth={} · cwd={}",
        provider.name(),
        credential.source,
        working_dir.display()
    );
    eprintln!("session: {}", session.path().display());

    let mut agent_config = AgentConfig::new(model, working_dir);
    agent_config.max_steps = max_steps;

    let mut line_open = false;
    let result = {
        let mut observer = |event: AgentEvent| match event {
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
            AgentEvent::ToolResult { name, ok, summary } => {
                eprintln!("← {name} {} {}", if ok { "ok" } else { "error" }, summary);
            }
            AgentEvent::TurnEnd { usage } => {
                if line_open {
                    println!();
                    line_open = false;
                }
                eprintln!(
                    "turn usage: in={} out={} cached={}",
                    usage.input_tokens, usage.output_tokens, usage.cached_tokens
                );
            }
        };
        let run = run_agent(
            &provider,
            &mut session,
            &agent_config,
            &prompt,
            &mut observer,
        );
        tokio::pin!(run);
        tokio::select! {
            result = &mut run => Some(result),
            _ = tokio::signal::ctrl_c() => None,
        }
    };

    if result.is_none() {
        let _ = session.append(&Event::SessionEnd { ts: now_ms() });
        eprintln!("\ncancelled");
        return Ok(());
    }

    let result = result.expect("checked above");
    if result.is_err() {
        eprintln!("session: {}", session.path().display());
    }
    if result.is_err() {
        eprintln!("session: {}", session.path().display());
    }

    let outcome = result?;
    if !outcome.final_text.ends_with('\n') {
        println!();
    }
    eprintln!(
        "done: {} turns, input={} output={} cached={} · {}",
        outcome.turns,
        outcome.usage.input_tokens,
        outcome.usage.output_tokens,
        outcome.usage.cached_tokens,
        session.path().display()
    );
    Ok(())
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
                    "no credential for {}; run `ni auth set {}` or set {}",
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
            let prompt = prompt.unwrap_or_else(|| "Reply with exactly: ni provider ok".into());
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
    if let Ok(home) = std::env::var("NI_HOME") {
        return PathBuf::from(home).join("sessions");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".ni").join("sessions");
    }
    PathBuf::from(".ni").join("sessions")
}

fn print_banner() {
    println!(
        "ni {} · minimal, observable coding agent",
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("status   M0 · provider");
    println!("docs     docs/index.html");
    println!("sessions {}", default_sessions_dir().display());
    println!();
    println!("auth     ni auth set deepseek");
    println!("next     ni exec \"find and fix a small bug\"");
    println!("         cargo xtask dashboard");
}
