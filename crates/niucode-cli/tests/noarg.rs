use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("niucode-cli-noarg-{tag}-{stamp}-{sequence}"));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create temp dir");
    directory
}

#[test]
fn bare_niucode_starts_the_tui() {
    let directory = temp_dir("tui");
    let home = directory.join("home");
    let work = directory.join("work");
    fs::create_dir_all(&home).expect("home");
    fs::create_dir_all(&work).expect("work");

    let mut child = Command::new(env!("CARGO_BIN_EXE_niucode"))
        .env("HOME", &home)
        .env("DEEPSEEK_API_KEY", "dummy")
        .env("NIUCODE_AUTH_DISABLE_KEYRING", "1")
        .env("TERM", "xterm-256color")
        .current_dir(&work)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn niucode");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(b"exit\n")
        .expect("write exit");
    let output = child.wait_with_output().expect("wait");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "stderr: {stderr}");
    assert!(stdout.contains("niucode ▸"), "stdout: {stdout:?}");
    assert!(stdout.contains("niucode tui"), "stdout: {stdout:?}");
    assert!(!stderr.contains("status   M3"), "old banner still printed");

    fs::remove_dir_all(directory).expect("cleanup");
}
