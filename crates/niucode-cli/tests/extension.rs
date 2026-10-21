use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "niucode-cli-ext-{tag}-{stamp}-{}",
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("work")).expect("dirs");
    fs::create_dir_all(dir.join("home")).expect("home");
    dir
}

#[test]
#[cfg(target_os = "linux")]
fn exec_mounts_calls_and_unmounts_a_sidecar() {
    check_extension_lifecycle(true);
}

#[test]
fn exec_records_extension_error_when_sandbox_is_missing() {
    check_extension_lifecycle(false);
}

fn check_extension_lifecycle(sandbox_available: bool) {
    let dir = temp_dir("exec");
    let work = dir.join("work");
    let sessions = dir.join("sessions");
    let home = dir.join("home");
    let bootstrap = Command::new(env!("CARGO_BIN_EXE_niucode"))
        .args(["session", "new"])
        .current_dir(&work)
        .env("NIUCODE_HOME", &home)
        .output()
        .expect("bootstrap workspace");
    assert!(
        bootstrap.status.success(),
        "bootstrap stderr: {}",
        String::from_utf8_lossy(&bootstrap.stderr)
    );
    let project = fs::read_dir(home.join("projects"))
        .expect("projects")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.is_dir())
        .expect("project workspace");
    let extensions = project.join("extensions");
    fs::create_dir_all(&extensions).expect("extensions");
    fs::write(
        extensions.join("demo.toml"),
        r#"
name = "demo"
command = "python3"
args = ["sidecar.py"]
timeout_secs = 10
"#,
    )
    .expect("manifest");
    fs::write(
        work.join("sidecar.py"),
        r#"import json, sys
for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "initialize":
        result = {"name": "demo", "version": "0.1.0", "tools": [{
            "name": "demo_echo",
            "description": "Echo text.",
            "input_schema": {"type": "object", "properties": {"text": {"type": "string"}}}
        }]}
    elif method == "tools/call":
        text = request.get("params", {}).get("arguments", {}).get("text", "")
        result = {"ok": True, "output": "echo: " + text}
    elif method == "shutdown":
        sys.exit(0)
    else:
        result = {"ok": False, "output": "unexpected"}
    print(json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "result": result}), flush=True)
"#,
    )
    .expect("sidecar");
    let fixture = dir.join("fixture.json");
    fs::write(
        &fixture,
        r#"{"schema_version":1,"turns":[[{"type":"tool_call_delta","index":0,"id":"ext_1","name":"demo_echo","arguments":"{\"text\":\"hello\"}"},{"type":"done","finish_reason":"tool_calls"}],[{"type":"text_delta","text":"done"},{"type":"done","finish_reason":"stop"}]]}"#,
    )
    .expect("fixture");

    let mut command = Command::new(env!("CARGO_BIN_EXE_niucode"));
    command
        .args([
            "exec",
            "use demo",
            "--replay",
            fixture.to_str().expect("fixture"),
            "--cwd",
            work.to_str().expect("work"),
            "--session-dir",
            sessions.to_str().expect("sessions"),
            "--sandbox",
            "full-access",
            "--approval",
            "never",
            "--network",
            "on",
        ])
        .env("NIUCODE_HOME", &home);
    if !sandbox_available {
        // Change only the child environment, so parallel tests remain isolated.
        let empty_path = dir.join("empty-path");
        fs::create_dir(&empty_path).expect("empty PATH");
        command.env("PATH", empty_path);
    }
    let output = command.output().expect("run niucode");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let session = fs::read_dir(&sessions)
        .expect("sessions")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .expect("session file");
    let log = fs::read_to_string(session).expect("session log");
    if sandbox_available {
        assert!(
            log.contains(r#""type":"extension_mount""#),
            "missing extension mount; stderr: {}\nsession: {log}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(log.contains(r#""output":"echo: hello""#));
        assert!(log.contains(r#""type":"extension_unmount""#));
        assert!(!log.contains(r#""type":"extension_error""#));
    } else {
        let events: Vec<serde_json::Value> = log
            .lines()
            .map(|line| serde_json::from_str(line).expect("event"))
            .collect();
        let error = events
            .iter()
            .find(|event| event["type"] == "extension_error")
            .expect("missing sandbox must be recorded as an extension error");
        assert!(error["message"]
            .as_str()
            .expect("message")
            .contains("spawn extension demo under bwrap"));
        assert!(!log.contains(r#""type":"extension_mount""#));
        assert!(!log.contains(r#""type":"extension_unmount""#));
        let result = events
            .iter()
            .find(|event| event["type"] == "tool_result")
            .expect("tool result");
        assert_eq!(result["ok"], false);
        assert_eq!(result["output"], "error: unknown tool \"demo_echo\"");
    }

    fs::remove_dir_all(dir).expect("cleanup");
}
