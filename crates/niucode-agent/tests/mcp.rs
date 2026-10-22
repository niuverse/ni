//! Agent-level test: a mounted MCP tool is advertised and dispatched.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use niucode_agent::{run_with_history, AgentConfig, ApprovalMode, ApprovalRequest};
use niucode_core::{load, Event, SessionLog};
use niucode_mcp::{McpHost, McpServerSpec};
use niucode_provider::{ScriptedProvider, StreamEvent, ToolCallDelta};
use serde_json::json;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../niucode-mcp/tests/fixture_server.py")
}

fn spec() -> McpServerSpec {
    let mut env = BTreeMap::new();
    env.insert(
        "PATH".to_string(),
        std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string()),
    );
    McpServerSpec {
        name: "fixture".to_string(),
        command: "python3".to_string(),
        args: vec![fixture().display().to_string()],
        env,
        read_only: true,
    }
}

#[tokio::test]
async fn a_turn_calls_a_mounted_mcp_tool() {
    let directory = tempfile::tempdir().unwrap();
    let host = Arc::new(
        McpHost::mount(&[spec()], Duration::from_secs(5))
            .await
            .expect("mount"),
    );
    let provider = ScriptedProvider::from_turns(vec![
        vec![
            StreamEvent::ToolCallDelta(ToolCallDelta {
                index: 0,
                id: Some("call-mcp".into()),
                name: Some("mcp__fixture__echo".into()),
                arguments: json!({"text": "hello mcp"}).to_string(),
            }),
            StreamEvent::Done {
                finish_reason: Some("tool_calls".into()),
            },
        ],
        vec![
            StreamEvent::TextDelta("the tool worked".into()),
            StreamEvent::Done {
                finish_reason: Some("stop".into()),
            },
        ],
    ]);
    let mut log = SessionLog::create(directory.path()).unwrap();
    let mut config = AgentConfig::new("scripted", directory.path());
    config.provider = "scripted".into();
    config.approval = ApprovalMode::Never;
    config.mcp_host = Some(host.clone());

    let outcome = run_with_history(
        &provider,
        &mut log,
        &config,
        "call the fixture",
        Vec::new(),
        &mut |_request: &ApprovalRequest| false,
        &mut |_event| {},
    )
    .await
    .unwrap();
    assert_eq!(outcome.final_text, "the tool worked");

    let loaded = load(log.path()).unwrap();
    let tool_output = loaded
        .events
        .iter()
        .find_map(|event| match event {
            Event::ToolResult { output, .. } => Some(output.as_str()),
            _ => None,
        })
        .expect("tool result");
    assert_eq!(tool_output, "hello mcp");
    host.shutdown().await.unwrap();
}
