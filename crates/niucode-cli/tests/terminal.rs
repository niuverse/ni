#[cfg(unix)]
#[test]
fn pty_editor_approval_details_resize_and_restoration() {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/terminal.py");
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

#[cfg(unix)]
#[test]
fn quiet_chat_keeps_partial_coverage_out_of_transcript_and_reuses_bodies() {
    let output = std::process::Command::new("python3")
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/volume.py"))
        .arg(env!("CARGO_BIN_EXE_niucode"))
        .output()
        .expect("Python 3 required for PTY regression");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn approval_preserves_history_and_requires_an_unmodified_shortcut() {
    let output = std::process::Command::new("python3")
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/approval_history.py"))
        .arg(env!("CARGO_BIN_EXE_niucode"))
        .output()
        .expect("Python 3 required for PTY regression");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
