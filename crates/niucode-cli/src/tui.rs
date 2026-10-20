//! Linear in-process TUI.
//!
//! It only consumes `AgentEvent`s and calls the same `run_with_history` entry
//! point as headless exec. No loop internals are imported here.

use std::io::{IsTerminal, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event as TerminalEvent, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, Clear, ClearType};
use niucode_agent::{
    project_history, run_with_history, AgentConfig, AgentEvent, ApprovalRequest, RunOutcome,
};
use niucode_core::{load, now_ms, Event, EventId, SessionLog, Usage};
use niucode_provider::{ChatMessage, Provider};
use niucode_tools::SandboxMode;

use crate::diff::{clean_text, render_plain};
use crate::editor::{Input, LineEditor};
use crate::goal::{self, GoalStatus, MAX_GOAL_TURNS};
use crate::review::{review_diff, ApprovalChoice};
use crate::session_ops;
use crate::slash::{
    self, Command as SlashCommand, CommandRegistry, GoalAction, Parsed as SlashParsed, PlanAction,
};
use crate::theme::{self, Theme, Tone};

const TOOL_OUTPUT_FOLD_LINES: usize = 12;
const TOOL_OUTPUT_FOLD_BYTES: usize = 2_000;
const WIDE_BANNER: usize = 62;

const LOGO: [&str; 6] = [
    "███╗   ██╗██╗██╗   ██╗ ██████╗ ██████╗ ██████╗ ███████╗",
    "████╗  ██║██║██║   ██║██╔════╝██╔═══██╗██╔══██╗██╔════╝",
    "██╔██╗ ██║██║██║   ██║██║     ██║   ██║██║  ██║█████╗",
    "██║╚██╗██║██║██║   ██║██║     ██║   ██║██║  ██║██╔══╝",
    "██║ ╚████║██║╚██████╔╝╚██████╗╚██████╔╝██████╔╝███████╗",
    "╚═╝  ╚═══╝╚═╝ ╚═════╝  ╚═════╝ ╚═════╝ ╚═════╝ ╚══════╝",
];

struct ToolEcho {
    name: String,
    summary: String,
    output: String,
}

#[derive(Debug, Clone, Default)]
struct SessionUiState {
    plan_mode: bool,
    goal: Option<String>,
    goal_run: bool,
    goal_turns: usize,
    pending_prompt: Option<String>,
}

impl SessionUiState {
    fn from_session(session: &SessionLog) -> Result<Self> {
        let loaded = load(session.path())?;
        Ok(Self::from_events(&loaded.events))
    }

    fn from_events(events: &[Event]) -> Self {
        let mut state = Self::default();
        for event in events {
            match event {
                Event::PlanMode { active, .. } => state.plan_mode = *active,
                Event::GoalSet { text, .. } => state.goal = Some(text.clone()),
                Event::GoalCleared { .. } => state.goal = None,
                _ => {}
            }
        }
        state
    }
}

pub async fn run(
    provider: &dyn Provider,
    session: &mut SessionLog,
    config: &AgentConfig,
    history: Vec<ChatMessage>,
) -> Result<()> {
    let out = Theme::detect_stdout();
    let err = Theme::detect_stderr();
    if std::io::stderr().is_terminal() {
        print_banner(err);
    }
    let commands = CommandRegistry::builtin();
    let mut editor = if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        Some(LineEditor::new(commands.clone()))
    } else {
        None
    };

    let mut state = SessionUiState::from_session(session)?;
    let mut history = history;
    let mut allow_session = false;
    let mut last_tool: Option<ToolEcho> = None;

    tokio::spawn(async {
        let mut last: Option<Instant> = None;
        loop {
            if tokio::signal::ctrl_c().await.is_err() {
                break;
            }
            let now = Instant::now();
            if last
                .map(|instant| now.duration_since(instant) < Duration::from_secs(2))
                .unwrap_or(false)
            {
                eprintln!("\nforce exit");
                std::process::exit(130);
            }
            last = Some(now);
        }
    });

    loop {
        let input = if let Some(prompt) = state.pending_prompt.take() {
            eprintln!(
                "{} {}",
                out.accent("↻ goal run"),
                out.dim(format!(
                    "turn {}/{} · {}",
                    state.goal_turns + 1,
                    MAX_GOAL_TURNS,
                    state.goal.as_deref().unwrap_or("")
                ))
            );
            prompt
        } else if let Some(editor) = &mut editor {
            println!();
            let raw_prompt = format!("niucode {} ", theme::PROMPT);
            let styled_prompt = format!("{} ", out.accent(format!("niucode {}", theme::PROMPT)));
            match editor.read_line(&raw_prompt, &styled_prompt, out)? {
                Input::Line(line) => line,
                Input::Interrupted => continue,
                Input::Eof => break,
            }
        } else {
            print!("\n{} ", out.accent(format!("niucode {}", theme::PROMPT)));
            std::io::stdout().flush()?;
            let mut input = String::new();
            if std::io::stdin().read_line(&mut input)? == 0 {
                break;
            }
            input
        };
        let prompt = input.trim();
        if prompt.is_empty() {
            continue;
        }
        match slash::parse(prompt) {
            SlashParsed::Command(command) => match handle_command(
                command,
                session,
                config,
                &mut history,
                &mut allow_session,
                &mut last_tool,
                &mut state,
                &commands,
                out,
                err,
            ) {
                Ok(CommandOutcome::Exit) => break,
                Ok(CommandOutcome::Continue) => continue,
                Err(error) => {
                    eprintln!(
                        "{} {}",
                        err.paint(Tone::Danger, theme::ERR),
                        err.paint(Tone::Danger, format!("{error:#}"))
                    );
                    continue;
                }
            },
            SlashParsed::Unknown(name) => {
                eprintln!(
                    "{} unknown command /{name}; try {}",
                    err.paint(Tone::Warning, theme::WARN),
                    err.paint(Tone::Bright, "/help")
                );
                continue;
            }
            SlashParsed::Usage(usage) => {
                eprintln!(
                    "{} usage: {}",
                    err.paint(Tone::Warning, theme::WARN),
                    err.paint(Tone::Bright, usage)
                );
                continue;
            }
            SlashParsed::None => {}
        }

        let mut line_open = false;
        let mut thinking = false;
        let mut text_started = false;
        let mut turn_no = 0_usize;
        let mut pending_usage: Option<Usage> = None;
        let mut tools_pending = 0_usize;

        let mut approver = |request: &ApprovalRequest| -> bool {
            if allow_session {
                eprintln!(
                    "{} {}",
                    err.paint(Tone::Success, theme::OK),
                    err.dim(format!("auto-approved · {}", clean_text(&request.summary)))
                );
                return true;
            }
            match prompt_approval(request, out, err) {
                ApprovalChoice::Allow => true,
                ApprovalChoice::Deny => false,
                ApprovalChoice::Session => {
                    allow_session = true;
                    true
                }
            }
        };

        let mut observer = |event: AgentEvent| match event {
            AgentEvent::TextDelta(delta) => {
                if delta.is_empty() {
                    return;
                }
                if !text_started {
                    if line_open {
                        println!();
                        line_open = false;
                    }
                    flush_usage(err, &mut pending_usage, turn_no);
                    print!("{} ", out.accent(theme::ASSISTANT));
                    text_started = true;
                    thinking = false;
                }
                print!("{delta}");
                let _ = std::io::stdout().flush();
                line_open = !delta.ends_with('\n');
            }
            AgentEvent::ReasoningDelta(_) => {
                if !thinking {
                    if line_open {
                        println!();
                        line_open = false;
                    }
                    eprintln!("{}", err.dim(format!("{} thinking…", theme::THINKING)));
                    thinking = true;
                }
            }
            AgentEvent::ToolCall { name, input } => {
                if line_open {
                    println!();
                    line_open = false;
                }
                text_started = false;
                thinking = false;
                tools_pending += 1;
                let detail = compact_json(&input);
                let mut line = format!(
                    "{} {}",
                    err.paint(Tone::Magenta, theme::TOOL),
                    err.paint(Tone::Bright, name)
                );
                if !detail.is_empty() {
                    line.push(' ');
                    line.push_str(&err.dim(detail));
                }
                eprintln!("{line}");
            }
            AgentEvent::ToolResult {
                name,
                ok,
                summary,
                output,
            } => {
                if line_open {
                    println!();
                    line_open = false;
                }
                text_started = false;
                thinking = false;
                render_tool_result(err, &name, ok, &summary, &output, &mut last_tool);
                tools_pending = tools_pending.saturating_sub(1);
                if tools_pending == 0 {
                    flush_usage(err, &mut pending_usage, turn_no);
                }
            }
            AgentEvent::TurnEnd { usage } => {
                if line_open {
                    println!();
                    line_open = false;
                }
                text_started = false;
                thinking = false;
                turn_no += 1;
                pending_usage = Some(usage);
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
                text_started = false;
                flush_usage(err, &mut pending_usage, turn_no);
                eprintln!(
                    "{} {}",
                    err.paint(Tone::Magenta, theme::COMPACT),
                    err.dim(format!(
                        "compacted · {replaced_events} events · ~{before_tokens} → ~{after_tokens} tokens"
                    ))
                );
            }
            AgentEvent::Warning(message) => {
                if line_open {
                    println!();
                    line_open = false;
                }
                text_started = false;
                flush_usage(err, &mut pending_usage, turn_no);
                eprintln!(
                    "{} {}",
                    err.paint(Tone::Warning, theme::WARN),
                    err.paint(Tone::Warning, clean_text(&message))
                );
            }
        };

        let mut run_config = config.clone();
        run_config.plan_mode = state.plan_mode;
        run_config.goal = state.goal.clone();
        if state.plan_mode {
            run_config.allowed_tools = Some(vec![
                "read".to_string(),
                "grep".to_string(),
                "glob".to_string(),
            ]);
        }

        let result = {
            let run = run_with_history(
                provider,
                session,
                &run_config,
                prompt,
                history.clone(),
                &mut approver,
                &mut observer,
            );
            tokio::pin!(run);
            tokio::select! {
                result = &mut run => Some(result),
                _ = tokio::signal::ctrl_c() => None,
            }
        };

        if line_open {
            println!();
        }
        flush_usage(err, &mut pending_usage, turn_no);

        match result {
            None => {
                state.goal_run = false;
                state.pending_prompt = None;
                let _ = session.append(&Event::SessionEnd { ts: now_ms() });
                history = project_history(&load(session.path())?.events)?;
                eprintln!(
                    "{}",
                    err.paint(
                        Tone::Warning,
                        format!("{} cancelled · Ctrl-C again within 2s to exit", theme::WARN)
                    )
                );
            }
            Some(Ok(outcome)) => {
                eprintln!(
                    "{} {}",
                    err.paint(Tone::Success, theme::OK),
                    err.dim(format!(
                        "done · {} turns · in {} · out {} · cache {}%",
                        outcome.turns,
                        outcome.usage.input_tokens,
                        outcome.usage.output_tokens,
                        outcome.usage.cache_hit_percent()
                    ))
                );
                history = project_history(&load(session.path())?.events)?;
                if state.goal_run {
                    if let Err(error) = goal_after_turn(
                        provider,
                        session,
                        &mut state,
                        &outcome,
                        &config.model,
                        out,
                        err,
                    )
                    .await
                    {
                        state.goal_run = false;
                        state.pending_prompt = None;
                        eprintln!(
                            "{} {}",
                            err.paint(Tone::Warning, theme::WARN),
                            err.paint(Tone::Warning, format!("goal evaluator failed: {error:#}"))
                        );
                    }
                }
            }
            Some(Err(error)) => {
                state.goal_run = false;
                state.pending_prompt = None;
                eprintln!(
                    "{} {}",
                    err.paint(Tone::Danger, theme::ERR),
                    err.paint(Tone::Danger, format!("{error:#}"))
                );
                history = project_history(&load(session.path())?.events)?;
            }
        }
    }

    Ok(())
}

enum CommandOutcome {
    Continue,
    Exit,
}

#[allow(clippy::too_many_arguments)]
fn handle_command(
    command: SlashCommand,
    session: &mut SessionLog,
    config: &AgentConfig,
    history: &mut Vec<ChatMessage>,
    allow_session: &mut bool,
    last_tool: &mut Option<ToolEcho>,
    state: &mut SessionUiState,
    commands: &CommandRegistry,
    out: Theme,
    err: Theme,
) -> Result<CommandOutcome> {
    match command {
        SlashCommand::Help => print_help(out, commands),
        SlashCommand::Last => print_last(out, last_tool.as_ref()),
        SlashCommand::Init => {
            if state.plan_mode {
                bail!("plan mode is read-only; use /plan off before /init");
            }
            run_init(config, out)?
        }
        SlashCommand::Plan(action) => handle_plan(session, state, action, out)?,
        SlashCommand::Goal(action) => handle_goal(session, state, action, out)?,
        SlashCommand::Clear => {
            let previous = session.path().to_path_buf();
            *session = new_session(session, config)?;
            history.clear();
            *allow_session = false;
            *last_tool = None;
            state.plan_mode = false;
            state.goal = None;
            state.goal_run = false;
            state.goal_turns = 0;
            state.pending_prompt = None;
            reset_screen(err);
            println!(
                "{} {}",
                out.paint(Tone::Success, theme::OK),
                out.dim(format!("new session · {}", session.path().display()))
            );
            println!("{}", out.dim(format!("previous · {}", previous.display())));
        }
        SlashCommand::Fork(at) => {
            let report = fork_session(session, at.as_deref())?;
            for warning in &report.warnings {
                eprintln!(
                    "{} {}",
                    err.paint(Tone::Warning, theme::WARN),
                    err.dim(warning)
                );
            }
            *session = SessionLog::open(&report.destination)?;
            let loaded = load(session.path())?;
            *history = project_history(&loaded.events)?;
            *state = SessionUiState::from_events(&loaded.events);
            *allow_session = false;
            *last_tool = None;
            reset_screen(err);
            println!(
                "{} {}",
                out.paint(Tone::Success, theme::OK),
                out.dim(format!(
                    "fork · {} · copied {} {}",
                    report.destination.display(),
                    report.copied,
                    if report.copied == 1 {
                        "event"
                    } else {
                        "events"
                    }
                ))
            );
            println!(
                "{}",
                out.dim(format!(
                    "at {} · parent {}",
                    report.at,
                    report.parent.display()
                ))
            );
        }
        SlashCommand::Status => print_status(session, config, out)?,
        SlashCommand::Session => print_session(session, out)?,
        SlashCommand::Exit => return Ok(CommandOutcome::Exit),
    }
    Ok(CommandOutcome::Continue)
}

fn handle_plan(
    session: &mut SessionLog,
    state: &mut SessionUiState,
    action: PlanAction,
    out: Theme,
) -> Result<()> {
    match action {
        PlanAction::Enter => {
            if state.plan_mode {
                println!("{}", out.dim("already in plan mode · /plan off to exit"));
                return Ok(());
            }
            state.plan_mode = true;
            session.append(&Event::PlanMode {
                active: true,
                ts: now_ms(),
            })?;
            println!(
                "{} {}",
                out.paint(Tone::Warning, theme::WARN),
                out.paint(Tone::Bright, "plan mode on · read-only")
            );
            println!(
                "{}",
                out.dim("  investigate and return a plan · /plan approve to exit")
            );
        }
        PlanAction::Exit | PlanAction::Approve => {
            if !state.plan_mode {
                println!("{}", out.dim("plan mode is already off"));
                return Ok(());
            }
            state.plan_mode = false;
            session.append(&Event::PlanMode {
                active: false,
                ts: now_ms(),
            })?;
            let label = if matches!(action, PlanAction::Approve) {
                "plan approved · execution available on the next prompt"
            } else {
                "plan mode off"
            };
            println!(
                "{} {}",
                out.paint(Tone::Success, theme::OK),
                out.paint(Tone::Bright, label)
            );
        }
        PlanAction::Status => {
            println!(
                "{} {}",
                out.accent("plan"),
                if state.plan_mode {
                    "on · read-only"
                } else {
                    "off"
                }
            );
        }
    }
    Ok(())
}

fn handle_goal(
    session: &mut SessionLog,
    state: &mut SessionUiState,
    action: GoalAction,
    out: Theme,
) -> Result<()> {
    match action {
        GoalAction::Show => match &state.goal {
            Some(goal) => {
                println!("{} {}", out.accent("goal"), out.paint(Tone::Bright, goal));
                if state.goal_run {
                    println!(
                        "{}",
                        out.dim(format!(
                            "  run active · turn {}/{} · /goal stop",
                            state.goal_turns, MAX_GOAL_TURNS
                        ))
                    );
                }
            }
            None => println!("{}", out.dim("no active goal · /goal <condition>")),
        },
        GoalAction::Run => {
            if state.plan_mode {
                bail!("goal run is disabled in plan mode; use /plan off first");
            }
            let Some(goal) = state.goal.clone() else {
                bail!("set a goal first: /goal <condition>");
            };
            state.goal_run = true;
            state.goal_turns = 0;
            state.pending_prompt = Some(goal::continue_prompt(&goal));
            println!(
                "{} {}",
                out.accent("goal run"),
                out.paint(Tone::Bright, format!("max {MAX_GOAL_TURNS} turns"))
            );
            println!("{}", out.dim("  /goal stop to stop after the current turn"));
        }
        GoalAction::Stop => {
            if state.goal_run {
                state.goal_run = false;
                state.pending_prompt = None;
                println!("{}", out.dim("goal run stopped"));
            } else {
                println!("{}", out.dim("goal run is not active"));
            }
        }
        GoalAction::Set(text) => {
            let text = text.trim();
            if text.is_empty() {
                println!("{}", out.dim("goal is empty; nothing changed"));
                return Ok(());
            }
            let text = text.chars().take(4_000).collect::<String>();
            session.append(&Event::GoalSet {
                text: text.clone(),
                ts: now_ms(),
            })?;
            state.goal = Some(text.clone());
            println!("{} {}", out.accent("goal"), out.paint(Tone::Bright, text));
            println!(
                "{}",
                out.dim("  persisted in the session · /goal clear to remove")
            );
        }
        GoalAction::Clear => {
            state.goal_run = false;
            state.pending_prompt = None;
            if state.goal.take().is_some() {
                session.append(&Event::GoalCleared { ts: now_ms() })?;
                println!("{} goal cleared", out.paint(Tone::Success, theme::OK));
            } else {
                println!("{}", out.dim("no active goal"));
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn goal_after_turn(
    provider: &dyn Provider,
    session: &mut SessionLog,
    state: &mut SessionUiState,
    outcome: &RunOutcome,
    model: &str,
    out: Theme,
    err: Theme,
) -> Result<()> {
    let Some(goal) = state.goal.clone() else {
        state.goal_run = false;
        return Ok(());
    };
    let verdict = goal::evaluate(provider, model, &goal, &outcome.final_text).await?;
    session.append(&Event::GoalVerdict {
        status: verdict.status.as_str().to_string(),
        reason: verdict.reason.clone(),
        ts: now_ms(),
    })?;

    match verdict.status {
        GoalStatus::Met => {
            state.goal_run = false;
            state.goal_turns = 0;
            state.pending_prompt = None;
            state.goal = None;
            session.append(&Event::GoalCleared { ts: now_ms() })?;
            println!(
                "{} {}",
                out.paint(Tone::Success, theme::OK),
                out.paint(
                    Tone::Bright,
                    format!("goal met · {}", clean_text(&verdict.reason))
                )
            );
        }
        GoalStatus::Impossible => {
            state.goal_run = false;
            state.pending_prompt = None;
            eprintln!(
                "{} {}",
                err.paint(Tone::Warning, theme::WARN),
                err.paint(
                    Tone::Warning,
                    format!("goal impossible · {}", clean_text(&verdict.reason))
                )
            );
        }
        GoalStatus::NotMet => {
            state.goal_turns += 1;
            if state.goal_turns >= MAX_GOAL_TURNS {
                state.goal_run = false;
                state.pending_prompt = None;
                eprintln!(
                    "{} {}",
                    err.paint(Tone::Warning, theme::WARN),
                    err.dim(format!(
                        "goal budget exhausted ({MAX_GOAL_TURNS} turns) · {}",
                        clean_text(&verdict.reason)
                    ))
                );
            } else {
                state.pending_prompt = Some(goal::continue_prompt(&goal));
                eprintln!(
                    "{} {}",
                    out.accent("↻ goal not met"),
                    out.dim(format!(
                        "turn {}/{} · {}",
                        state.goal_turns,
                        MAX_GOAL_TURNS,
                        clean_text(&verdict.reason)
                    ))
                );
            }
        }
    }
    Ok(())
}

fn run_init(config: &AgentConfig, out: Theme) -> Result<()> {
    if config.sandbox.mode == SandboxMode::ReadOnly {
        bail!("sandbox is read-only; cannot write AGENTS.md");
    }
    let path = config.working_dir.join("AGENTS.md");
    if let Ok(metadata) = std::fs::symlink_metadata(&path) {
        if metadata.is_file() || metadata.file_type().is_symlink() {
            println!(
                "{} {}",
                out.paint(Tone::Warning, theme::WARN),
                out.dim(format!(
                    "{} already exists · not overwriting",
                    path.display()
                ))
            );
            return Ok(());
        }
        bail!("refusing to overwrite non-file path {}", path.display());
    }

    let template = slash::init_template(&config.working_dir);
    std::fs::write(&path, template).with_context(|| format!("write {}", path.display()))?;
    println!(
        "{} {}",
        out.paint(Tone::Success, theme::OK),
        out.dim(format!("wrote {} · next turn will load it", path.display()))
    );
    Ok(())
}

fn new_session(session: &SessionLog, config: &AgentConfig) -> Result<SessionLog> {
    let directory = session
        .path()
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut new_session = SessionLog::create(directory)?;
    new_session.append(&Event::SessionStart {
        id: new_session.id().to_string(),
        cwd: config.working_dir.display().to_string(),
        model: config.model.clone(),
        ts: now_ms(),
    })?;
    new_session.append(&Event::Policy {
        sandbox: config.sandbox.mode.as_str().into(),
        approval: config.approval.as_str().into(),
        network: config.sandbox.network.as_str().into(),
        backend: config.sandbox.backend().into(),
        ts: now_ms(),
    })?;
    Ok(new_session)
}

fn fork_session(session: &SessionLog, at: Option<&str>) -> Result<session_ops::ForkReport> {
    let source = session.path().to_path_buf();
    let at = match at {
        Some(at) => at.to_string(),
        None => {
            let loaded = load(&source)?;
            if loaded.events.is_empty() {
                bail!("current session has no events to fork");
            }
            EventId::from_index(loaded.events.len() - 1).to_string()
        }
    };
    let directory = source
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    session_ops::fork(&source, &at, directory)
}

fn print_status(session: &SessionLog, config: &AgentConfig, out: Theme) -> Result<()> {
    let loaded = load(session.path())?;
    let mut usage = Usage::default();
    let mut turns = 0_usize;
    let mut compactions = 0_usize;
    let mut instructions = 0_usize;
    let mut subagents = 0_usize;
    let mut extensions = 0_usize;
    for event in &loaded.events {
        match event {
            Event::UserMessage { .. } => turns += 1,
            Event::Compaction { .. } => compactions += 1,
            Event::Instructions { .. } => instructions += 1,
            Event::SubagentSpawn { .. } => subagents += 1,
            Event::ExtensionMount { .. } => extensions += 1,
            Event::TurnEnd { usage: turn, .. } => {
                usage.input_tokens += turn.input_tokens;
                usage.output_tokens += turn.output_tokens;
                usage.cached_tokens += turn.cached_tokens;
                usage.reasoning_tokens += turn.reasoning_tokens;
            }
            _ => {}
        }
    }

    println!("{}", out.accent("status"));
    println!("  model      {}", config.model);
    println!("  cwd        {}", config.working_dir.display());
    println!(
        "  sandbox    {} · network {} · approval {} · backend {}",
        config.sandbox.mode.as_str(),
        config.sandbox.network.as_str(),
        config.approval.as_str(),
        config.sandbox.backend()
    );
    println!(
        "  context    budget {} · trigger {}% · keep {} turns",
        config.context.budget_tokens,
        config.context.compact_at_percent,
        config.context.keep_recent_turns
    );
    let ui = SessionUiState::from_events(&loaded.events);
    println!("  session    {}", session.path().display());
    println!(
        "  plan       {}",
        if ui.plan_mode {
            "on · read-only"
        } else {
            "off"
        }
    );
    println!("  goal       {}", ui.goal.as_deref().unwrap_or("none"));
    println!(
        "  events     {} decoded · {} warnings",
        loaded.events.len(),
        loaded.warnings.len()
    );
    println!(
        "  turns      {turns} · compactions {compactions} · instructions {instructions} · subagents {subagents} · extensions {extensions}"
    );
    if usage.input_tokens > 0 {
        println!(
            "  cache      {}% · in {} · out {} · cached {}",
            usage.cache_hit_percent(),
            usage.input_tokens,
            usage.output_tokens,
            usage.cached_tokens
        );
    }
    Ok(())
}

fn print_session(session: &SessionLog, out: Theme) -> Result<()> {
    let loaded = load(session.path())?;
    let turns = loaded
        .events
        .iter()
        .filter(|event| matches!(event, Event::UserMessage { .. }))
        .count();
    println!("{}", out.accent("session"));
    println!("  path       {}", session.path().display());
    println!(
        "  events     {} decoded · {} warnings",
        loaded.events.len(),
        loaded.warnings.len()
    );
    println!("  turns      {turns}");
    Ok(())
}

fn reset_screen(theme: Theme) {
    if std::io::stdout().is_terminal() {
        let mut out = std::io::stdout();
        let _ = execute!(out, Clear(ClearType::All), MoveTo(0, 0));
    }
    if std::io::stderr().is_terminal() {
        eprintln!(
            "{}",
            theme.accent(format!(
                "NIUCODE // v{} · fresh session",
                env!("CARGO_PKG_VERSION")
            ))
        );
        eprintln!(
            "{}",
            theme.dim("linear session · :help commands · Ctrl-C cancels the turn")
        );
    }
}

fn prompt_approval(request: &ApprovalRequest, out: Theme, err: Theme) -> ApprovalChoice {
    if let Some(preview) = &request.preview {
        if preview.is_diff() && std::io::stdout().is_terminal() && std::io::stderr().is_terminal() {
            match review_diff(request, preview, out) {
                Ok(Some(choice)) => return choice,
                Ok(None) => {}
                Err(error) => eprintln!(
                    "{} {}",
                    err.paint(Tone::Warning, theme::WARN),
                    err.dim(format!("diff pager unavailable: {error}"))
                ),
            }
        }
        println!();
        eprintln!(
            "{} {}",
            err.paint(Tone::Magenta, "approval"),
            err.paint(Tone::Bright, clean_text(&request.summary))
        );
        eprint!("{}", render_plain(preview, plain_width()));
    } else {
        println!();
        eprintln!(
            "{} {}",
            err.paint(Tone::Magenta, "approval"),
            err.paint(Tone::Bright, clean_text(&request.summary))
        );
    }

    prompt_keys(err)
}

fn prompt_keys(err: Theme) -> ApprovalChoice {
    if terminal::enable_raw_mode().is_ok() {
        eprint!(
            "{}  {}  {}  ",
            err.paint(Tone::Accent, "[a]llow"),
            err.paint(Tone::Danger, "[d]eny"),
            err.paint(Tone::Magenta, "[s]ession")
        );
        let _ = std::io::stderr().flush();
        let choice = loop {
            match event::read() {
                Ok(TerminalEvent::Key(KeyEvent {
                    code, modifiers, ..
                })) => match code {
                    KeyCode::Char('a') | KeyCode::Char('A') => {
                        break ApprovalChoice::Allow;
                    }
                    KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Esc => {
                        break ApprovalChoice::Deny;
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        break ApprovalChoice::Session;
                    }
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                        break ApprovalChoice::Deny;
                    }
                    _ => {}
                },
                Ok(_) => {}
                Err(_) => break ApprovalChoice::Deny,
            }
        };
        let _ = terminal::disable_raw_mode();
        eprintln!();
        return choice;
    }

    eprint!(
        "{} {} {}: ",
        err.paint(Tone::Accent, "[a]llow"),
        err.paint(Tone::Danger, "[d]eny"),
        err.paint(Tone::Magenta, "[s]ession")
    );
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return ApprovalChoice::Deny;
    }
    match line.trim() {
        "a" | "A" | "allow" | "yes" | "y" => ApprovalChoice::Allow,
        "s" | "S" | "session" => ApprovalChoice::Session,
        _ => ApprovalChoice::Deny,
    }
}

fn plain_width() -> usize {
    terminal::size()
        .map(|(columns, _)| columns.saturating_sub(1) as usize)
        .unwrap_or(100)
        .max(40)
}

fn flush_usage(theme: Theme, pending: &mut Option<Usage>, turn_no: usize) {
    let Some(usage) = pending.take() else {
        return;
    };
    eprintln!(
        "{}",
        theme.dim(format!(
            "{} turn {} · in {} · out {} · cache {}%",
            theme::TURN,
            turn_no,
            usage.input_tokens,
            usage.output_tokens,
            usage.cache_hit_percent()
        ))
    );
}

fn render_tool_result(
    theme: Theme,
    name: &str,
    ok: bool,
    summary: &str,
    output: &str,
    last: &mut Option<ToolEcho>,
) {
    let (symbol, tone) = if ok {
        (theme::OK, Tone::Success)
    } else {
        (theme::ERR, Tone::Danger)
    };
    let summary = clean_text(summary);
    eprintln!(
        "{} {} {}",
        theme.paint(tone, symbol),
        theme.paint(Tone::Bright, name),
        theme.dim(&summary)
    );

    let body = output_body(summary.as_str(), output);
    if !body.is_empty() {
        if should_fold(&body, output.len()) {
            eprintln!(
                "{} {}",
                theme.dim(theme::BRANCH),
                theme.dim(format!(
                    "{} lines · {} · :last to expand",
                    body.len(),
                    human_bytes(output.len())
                ))
            );
        } else {
            for line in &body {
                eprintln!("{} {}", theme.dim(theme::BRANCH), clean_text(line));
            }
        }
    }
    *last = Some(ToolEcho {
        name: name.to_string(),
        summary,
        output: output.to_string(),
    });
}

fn output_body<'a>(summary: &str, output: &'a str) -> Vec<&'a str> {
    let body = if output == summary {
        ""
    } else {
        match output.split_once('\n') {
            Some((first, rest)) if first == summary => rest,
            _ => output,
        }
    };
    body.lines().collect()
}

fn should_fold(lines: &[&str], bytes: usize) -> bool {
    lines.len() > TOOL_OUTPUT_FOLD_LINES || bytes > TOOL_OUTPUT_FOLD_BYTES
}

fn human_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn print_banner(theme: Theme) {
    let width = terminal::size()
        .map(|(columns, _)| columns as usize)
        .unwrap_or(80)
        .max(1);
    let lines = banner_lines(width);
    let logo_lines = if width >= WIDE_BANNER { LOGO.len() } else { 1 };

    eprintln!();
    for (index, line) in lines.iter().enumerate() {
        if index < logo_lines {
            eprintln!("{}", theme.accent(line));
        } else {
            eprintln!("{}", theme.dim(line));
        }
    }
    eprintln!();
}

fn banner_lines(width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    if width >= WIDE_BANNER {
        lines.extend(LOGO.iter().map(|line| (*line).to_string()));
        lines.push(format!(
            "minimal · observable · event-sourced · v{}",
            env!("CARGO_PKG_VERSION")
        ));
    } else {
        lines.push(format!(
            "NIUCODE // minimal, observable coding agent · v{}",
            env!("CARGO_PKG_VERSION")
        ));
    }
    lines.push("linear session · :help commands · Ctrl-C cancels the turn".to_string());
    lines
}

fn print_last(theme: Theme, last: Option<&ToolEcho>) {
    let Some(echo) = last else {
        eprintln!("{}", theme.dim("no tool output yet"));
        return;
    };
    println!();
    println!(
        "{}",
        theme.paint(
            Tone::Magenta,
            format!(
                "╭─ last tool · {} · {}",
                echo.name,
                clean_text(&echo.summary)
            )
        )
    );
    if echo.output.is_empty() {
        println!("{} {}", theme.dim(theme::BRANCH), theme.dim("(no output)"));
    } else {
        for line in echo.output.lines() {
            println!("{} {}", theme.dim(theme::BRANCH), clean_text(line));
        }
    }
    println!("{}", theme.dim("╰─ end"));
}

fn print_help(theme: Theme, commands: &CommandRegistry) {
    println!("{}", theme.accent("commands"));
    for spec in commands.all() {
        println!(
            "  {} {}",
            theme.paint(Tone::Bright, format!("{:<9}", spec.name)),
            spec.description
        );
    }
    println!();
    println!(
        "{}",
        theme.dim("  aliases: exit · quit · :help · :last · :q")
    );
    println!(
        "{}",
        theme.dim("  other text (including plain prompts) goes to the model")
    );
}

fn compact_json(value: &serde_json::Value) -> String {
    let rendered = serde_json::to_string(value).unwrap_or_default();
    clean_text(&rendered).chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_the_summary_line_from_tool_output() {
        assert_eq!(
            output_body("exit: 1", "exit: 1\nstdout:\nhello\n"),
            vec!["stdout:", "hello"]
        );
        assert!(output_body("exit: 1", "exit: 1").is_empty());
        assert_eq!(output_body("summary", "other\n"), vec!["other"]);
    }

    #[test]
    fn folds_only_past_the_threshold() {
        let short = vec!["line"; TOOL_OUTPUT_FOLD_LINES];
        assert!(!should_fold(&short, 10));
        let long = vec!["line"; TOOL_OUTPUT_FOLD_LINES + 1];
        assert!(should_fold(&long, 10));
        assert!(should_fold(&["line"], TOOL_OUTPUT_FOLD_BYTES + 1));
    }

    #[test]
    fn human_bytes_is_stable() {
        assert_eq!(human_bytes(12), "12 B");
        assert_eq!(human_bytes(2_048), "2.0 KB");
        assert_eq!(human_bytes(2 * 1024 * 1024), "2.0 MB");
    }

    #[test]
    fn session_ui_state_uses_the_latest_plan_and_goal_events() {
        let state = SessionUiState::from_events(&[
            Event::PlanMode {
                active: true,
                ts: 1,
            },
            Event::GoalSet {
                text: "all tests pass".into(),
                ts: 2,
            },
            Event::PlanMode {
                active: false,
                ts: 3,
            },
        ]);
        assert!(!state.plan_mode);
        assert_eq!(state.goal.as_deref(), Some("all tests pass"));

        let state = SessionUiState::from_events(&[
            Event::PlanMode {
                active: true,
                ts: 1,
            },
            Event::GoalSet {
                text: "all tests pass".into(),
                ts: 2,
            },
            Event::GoalCleared { ts: 3 },
        ]);
        assert!(state.plan_mode);
        assert!(state.goal.is_none());
    }

    #[test]
    fn banner_switches_between_compact_and_logo() {
        let compact = banner_lines(40);
        assert_eq!(compact.len(), 2);
        assert!(compact[0].contains("NIUCODE"));

        let wide = banner_lines(100);
        assert!(wide.len() > compact.len());
        assert!(wide[0].contains("███"));
    }
}
