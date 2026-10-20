use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use ni_core::{now_ms, Event, SessionLog};

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

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Session { command }) => run_session(command),
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
    println!("status   M0 · bootstrap");
    println!("docs     docs/index.html");
    println!("sessions {}", default_sessions_dir().display());
    println!();
    println!("next     ni session new");
    println!("         cargo xtask dashboard");
}
