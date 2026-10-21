#![cfg(unix)]

fn run_scenario(script: &str) {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(script);
    let output = std::process::Command::new("python3")
        .arg(script)
        .arg(env!("CARGO_BIN_EXE_niucode"))
        .output()
        .expect("Python 3 is required for the Unix PTY regression test");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn pty_editor_approval_details_resize_and_restoration() {
    run_scenario("terminal.py");
}

#[test]
fn quiet_chat_keeps_partial_coverage_out_of_transcript_and_reuses_bodies() {
    run_scenario("volume.py");
}

#[test]
fn approval_preserves_history_and_requires_an_unmodified_shortcut() {
    run_scenario("approval_history.py");
}
