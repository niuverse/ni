//! Linear in-process TUI.
//!
//! It only consumes `AgentEvent`s and calls the same `run_with_history` entry
//! point as headless exec. No loop internals are imported here.

use std::io::Write;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event as TerminalEvent, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;
use niucode_agent::{project_history, run_with_history, AgentConfig, AgentEvent, ApprovalRequest};
use niucode_core::{load, now_ms, Event, SessionLog};
use niucode_provider::{ChatMessage, Provider};

pub async fn run(
    provider: &dyn Provider,
    session: &mut SessionLog,
    config: &AgentConfig,
    history: Vec<ChatMessage>,
) -> Result<()> {
    let mut history = history;
    let mut allow_session = false;

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
        print!("\nni> ");
        std::io::stdout().flush()?;
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input)? == 0 {
            break;
        }
        let prompt = input.trim();
        if prompt.is_empty() {
            continue;
        }
        if matches!(prompt, "exit" | "quit" | ":q") {
            break;
        }

        let mut line_open = false;
        let mut thinking = false;

        let mut approver = |request: &ApprovalRequest| -> bool {
            if allow_session {
                eprintln!("\nauto-approved for this session: {}", request.summary);
                return true;
            }
            match prompt_approval(request) {
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
                    eprintln!("thinking…");
                    thinking = true;
                }
            }
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

        let result = {
            let run = run_with_history(
                provider,
                session,
                config,
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

        match result {
            None => {
                let _ = session.append(&Event::SessionEnd { ts: now_ms() });
                history = project_history(&load(session.path())?.events)?;
                eprintln!("cancelled; Ctrl-C again within 2s to exit");
            }
            Some(Ok(outcome)) => {
                eprintln!(
                    "done: {} turns, input={} output={} cached={}",
                    outcome.turns,
                    outcome.usage.input_tokens,
                    outcome.usage.output_tokens,
                    outcome.usage.cached_tokens
                );
                history = project_history(&load(session.path())?.events)?;
            }
            Some(Err(error)) => {
                eprintln!("error: {error:#}");
                history = project_history(&load(session.path())?.events)?;
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApprovalChoice {
    Allow,
    Deny,
    Session,
}

fn prompt_approval(request: &ApprovalRequest) -> ApprovalChoice {
    println!();
    eprintln!("approval required: {}", request.summary);
    if let Some(preview) = &request.preview {
        eprintln!("{preview}");
    }

    if terminal::enable_raw_mode().is_ok() {
        eprint!("[a]llow  [d]eny  [s]ession  ");
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

    eprint!("[a]llow [d]eny [s]ession: ");
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

fn compact_json(value: &serde_json::Value) -> String {
    let rendered = serde_json::to_string(value).unwrap_or_default();
    rendered.chars().take(200).collect()
}
