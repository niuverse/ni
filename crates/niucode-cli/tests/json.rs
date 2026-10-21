use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[test]
fn exec_tool_allowlist_blocks_replayed_mutation() {
    let directory = temp_dir("allowlist");
    let work = directory.join("work");
    fs::create_dir_all(&work).unwrap();
    let replay = directory.join("fixture.json");
    fs::write(
        &replay,
        serde_json::json!({
            "schema_version": 1,
            "turns": [
                [
                    {"type":"tool_call_delta","index":0,"id":"write-1","name":"write",
                     "arguments":"{\"path\":\"forbidden.txt\",\"content\":\"unexpected\"}"},
                    {"type":"done","finish_reason":"tool_calls"}
                ],
                [{"type":"text_delta","text":"finished"},{"type":"done","finish_reason":"stop"}]
            ]
        })
        .to_string(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_niucode"))
        .args([
            "exec",
            "try writing",
            "--json",
            "--tools",
            "read,grep,glob",
            "--replay",
        ])
        .arg(&replay)
        .arg("--cwd")
        .arg(&work)
        .arg("--session-dir")
        .arg(directory.join("sessions"))
        .args([
            "--approval",
            "never",
            "--sandbox",
            "full-access",
            "--network",
            "on",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!work.join("forbidden.txt").exists());
    let events: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(events
        .iter()
        .any(|v| v["type"] == "tool_result" && v["ok"] == false));
    fs::remove_dir_all(directory).unwrap();
}

fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("niucode-cli-json-{tag}-{stamp}-{sequence}"));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create temp dir");
    directory
}

fn fixture(directory: &Path) -> PathBuf {
    let path = directory.join("fixture.json");
    fs::write(
        &path,
        r#"{
  "schema_version": 1,
  "turns": [
    [
      {"type": "text_delta", "text": "hello"},
      {"type": "usage", "usage": {"input_tokens": 10, "output_tokens": 2, "cached_tokens": 5, "reasoning_tokens": 0}},
      {"type": "done", "finish_reason": "stop"}
    ]
  ]
}
"#,
    )
    .expect("write fixture");
    path
}

#[test]
fn exec_json_emits_a_versioned_jsonl_stream() {
    let directory = temp_dir("stream");
    let work = directory.join("work");
    fs::create_dir_all(&work).expect("work dir");
    let fixture = fixture(&directory);
    let sessions = directory.join("sessions");

    let output = Command::new(env!("CARGO_BIN_EXE_niucode"))
        .args([
            "exec",
            "hello",
            "--json",
            "--replay",
            fixture.to_str().expect("fixture path"),
            "--cwd",
            work.to_str().expect("work path"),
            "--session-dir",
            sessions.to_str().expect("sessions path"),
            "--approval",
            "never",
            "--sandbox",
            "full-access",
            "--network",
            "on",
        ])
        .output()
        .expect("run niucode");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let events = stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("json line"))
        .collect::<Vec<_>>();
    assert_eq!(events[0]["type"], "start");
    assert_eq!(events[0]["protocol"], 1);
    assert!(events[0]["session"].as_str().is_some());
    assert!(events
        .iter()
        .any(|event| event["type"] == "text_delta" && event["text"] == "hello"));
    assert!(events
        .iter()
        .any(|event| event["type"] == "turn_end" && event["usage"]["cached_tokens"] == 5));
    assert_eq!(events.last().expect("last")["type"], "done");
    assert_eq!(events.last().expect("last")["usage"]["cached_tokens"], 5);

    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn exec_json_refuses_interactive_approval() {
    let directory = temp_dir("approval");
    let fixture = fixture(&directory);
    let sessions = directory.join("sessions");

    let output = Command::new(env!("CARGO_BIN_EXE_niucode"))
        .args([
            "exec",
            "hello",
            "--json",
            "--replay",
            fixture.to_str().expect("fixture path"),
            "--cwd",
            directory.to_str().expect("cwd"),
            "--session-dir",
            sessions.to_str().expect("sessions path"),
            "--approval",
            "ask",
            "--sandbox",
            "full-access",
            "--network",
            "on",
        ])
        .output()
        .expect("run niucode");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--json cannot prompt for approval"),
        "{stderr}"
    );

    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn exec_json_reports_truncation_as_error_and_preserves_usage() {
    let directory = temp_dir("length");
    let fixture = directory.join("length.json");
    fs::write(
        &fixture,
        serde_json::json!({
            "schema_version": 1,
            "turns": [[
                {"type": "reasoning_delta", "text": "still working"},
                {"type": "usage", "usage": {"input_tokens": 2969, "output_tokens": 8192,
                    "cached_tokens": 1920, "reasoning_tokens": 8192}},
                {"type": "done", "finish_reason": "length"}
            ]]
        })
        .to_string(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_niucode"))
        .args(["exec", "implement transpose", "--json", "--replay"])
        .arg(&fixture)
        .arg("--cwd")
        .arg(&directory)
        .arg("--session-dir")
        .arg(directory.join("sessions"))
        .args([
            "--approval",
            "never",
            "--sandbox",
            "full-access",
            "--network",
            "on",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let events: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events.last().unwrap()["type"], "error");
    assert!(events.last().unwrap()["message"]
        .as_str()
        .unwrap()
        .contains("finish_reason=length"));
    assert!(!events.iter().any(|e| e["type"] == "done"));
    assert!(events
        .iter()
        .any(|e| e["type"] == "turn_end" && e["usage"]["output_tokens"] == 8192));
    let session = events[0]["session"].as_str().unwrap();
    let log = niucode_core::load(Path::new(session)).unwrap().events;
    assert!(
        matches!(log.last(), Some(niucode_core::Event::TaskFinished { status, .. }) if status == "error")
    );
    assert!(log.iter().any(|e| matches!(e, niucode_core::Event::TurnEnd { finish_reason: Some(reason), .. } if reason == "length")));
    fs::remove_dir_all(directory).unwrap();
}
