//! Offline PTY regression for TUI Markdown rendering.
//!
//! Linux-only: the fixture passes `--sandbox read-only`, which needs bubblewrap.
//! macOS runners do not provide bwrap, so the child exits before the TUI starts.

#![cfg(all(unix, target_os = "linux"))]

use std::path::Path;
use std::process::Command;

#[test]
fn pty_markdown_renders_emphasis_and_tables() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/tui_markdown.py");
    let output = Command::new("python3")
        .arg(script)
        .arg(env!("CARGO_BIN_EXE_niucode"))
        .output()
        .expect("python3 is required for the Unix PTY markdown test");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
