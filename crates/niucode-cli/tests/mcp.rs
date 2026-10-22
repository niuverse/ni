//! End-to-end MCP mount from a trust file through `exec`.

use std::path::{Path, PathBuf};

use niucode::ExecOptions;
use serde_json::json;

fn fixture_server() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../niucode-mcp/tests/fixture_server.py")
}

#[tokio::test]
async fn exec_mounts_a_read_only_server_from_config() {
    let directory = tempfile::tempdir().expect("tempdir");
    let config = directory.path().join("mcp.toml");
    std::fs::write(
        &config,
        format!(
            r#"
[[server]]
name = "fixture"
command = "python3"
args = ["{}"]
read_only = true

[server.env]
PATH = "{}"
"#,
            fixture_server().display(),
            std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into())
        ),
    )
    .expect("config");

    let replay = directory.path().join("replay.json");
    std::fs::write(
        &replay,
        json!({
            "schema_version": 1,
            "turns": [
                [{
                    "type": "tool_call_delta",
                    "index": 0,
                    "id": "call-mcp",
                    "name": "mcp__fixture__echo",
                    "arguments": json!({"text": "from cli"}).to_string()
                }],
                [{"type": "text_delta", "text": "done"}]
            ]
        })
        .to_string(),
    )
    .expect("replay");

    let options = ExecOptions {
        prompt: "call the fixture".into(),
        cwd: Some(directory.path().to_path_buf()),
        session_dir: Some(directory.path().join("sessions")),
        replay: Some(replay),
        sandbox: "full-access".into(),
        network: "on".into(),
        approval: "never".into(),
        max_steps: 2,
        mcp_config: Some(config),
        ..ExecOptions::default()
    };
    let code = niucode::run_exec_with_hooks(options, &mut niucode::NoHooks)
        .await
        .expect("exec");
    assert_eq!(code, 0);

    let sessions = std::fs::read_dir(directory.path().join("sessions")).expect("sessions");
    let mut outputs = Vec::new();
    for entry in sessions.flatten() {
        let text = std::fs::read_to_string(entry.path()).expect("session");
        for line in text.lines() {
            let event: serde_json::Value = serde_json::from_str(line).expect("event");
            if event["type"] == "tool_result" {
                outputs.push(event["output"].as_str().unwrap_or("").to_string());
            }
        }
    }
    assert!(
        outputs.iter().any(|output| output == "from cli"),
        "{outputs:?}"
    );
}
