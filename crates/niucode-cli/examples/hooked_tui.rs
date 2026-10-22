//! Minimal embedding probe: run the TUI with recording hooks.
//!
//! The integration harness sets `NIUCODE_HOOKS_ROOT`, `NIUCODE_HOOKS_REPLAY`,
//! and `NIUCODE_HOOKS_MARKER`, then drives this binary through a PTY.

use std::io::Write;
use std::path::PathBuf;

use niucode::{AgentEvent, RunOutcome, TuiOptions, TurnHooks};

struct Probe {
    marker: PathBuf,
}

impl Probe {
    fn record(&self, line: &str) {
        let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.marker)
        else {
            return;
        };
        let _ = writeln!(file, "{line}");
    }
}

impl TurnHooks for Probe {
    fn augment_system_prompt(&mut self, prompt: &mut String) {
        self.record("prompt");
        prompt.push_str("\n<!-- hook probe -->");
    }

    fn on_event(&mut self, _event: &AgentEvent) {
        self.record("event");
    }

    fn on_turn_end(&mut self, _outcome: &RunOutcome) {
        self.record("turn_end");
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let root = PathBuf::from(std::env::var("NIUCODE_HOOKS_ROOT")?);
    let replay = PathBuf::from(std::env::var("NIUCODE_HOOKS_REPLAY")?);
    let marker = PathBuf::from(std::env::var("NIUCODE_HOOKS_MARKER")?);
    let options = TuiOptions {
        replay: Some(replay),
        cwd: Some(root.join("work")),
        session_dir: Some(root.join("sessions")),
        context_budget: 64_000,
        sandbox: "full-access".into(),
        network: "on".into(),
        approval: "never".into(),
        ..TuiOptions::default()
    };
    niucode::run_tui_with_hooks(options, Box::new(Probe { marker })).await
}
