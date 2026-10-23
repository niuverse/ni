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
        write_tools: Vec::new(),
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

fn writable_spec() -> McpServerSpec {
    let mut spec = spec();
    spec.read_only = false;
    spec.write_tools = vec!["echo".to_string()];
    spec
}

fn scripted_call(name: &str, arguments: serde_json::Value) -> ScriptedProvider {
    ScriptedProvider::from_turns(vec![
        vec![
            StreamEvent::ToolCallDelta(ToolCallDelta {
                index: 0,
                id: Some("call-write".into()),
                name: Some(name.to_string()),
                arguments: arguments.to_string(),
            }),
            StreamEvent::Done {
                finish_reason: Some("tool_calls".into()),
            },
        ],
        vec![
            StreamEvent::TextDelta("finished".into()),
            StreamEvent::Done {
                finish_reason: Some("stop".into()),
            },
        ],
    ])
}

#[tokio::test]
async fn a_mutating_mcp_call_is_approved_before_dispatch() {
    let directory = tempfile::tempdir().unwrap();
    let host = Arc::new(
        McpHost::mount_with(&[writable_spec()], Duration::from_secs(5), true)
            .await
            .expect("write mount"),
    );
    let provider = scripted_call("mcp__fixture__echo", json!({"text": "approved write"}));
    let mut log = SessionLog::create(directory.path()).unwrap();
    let mut config = AgentConfig::new("scripted", directory.path());
    config.provider = "scripted".into();
    config.approval = ApprovalMode::Ask;
    config.mcp_host = Some(host.clone());
    let mut seen: Option<ApprovalRequest> = None;

    let outcome = run_with_history(
        &provider,
        &mut log,
        &config,
        "call the write tool",
        Vec::new(),
        &mut |request: &ApprovalRequest| {
            seen = Some(request.clone());
            true
        },
        &mut |_event| {},
    )
    .await
    .unwrap();
    assert_eq!(outcome.final_text, "finished");
    let request = seen.expect("approval requested");
    assert_eq!(request.tool, "mcp__fixture__echo");
    assert!(
        request.summary.contains("mutating MCP"),
        "{}",
        request.summary
    );
    assert!(
        request.summary.contains("approved write"),
        "{}",
        request.summary
    );

    let loaded = load(log.path()).unwrap();
    assert!(loaded.events.iter().any(|event| matches!(
        event,
        Event::Approval { tool, allowed: true, .. } if tool == "mcp__fixture__echo"
    )));
    assert!(loaded.events.iter().any(|event| matches!(
        event,
        Event::ToolResult { output, .. } if output == "approved write"
    )));
    host.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_denied_mutation_writes_no_tool_result_output() {
    let directory = tempfile::tempdir().unwrap();
    let host = Arc::new(
        McpHost::mount_with(&[writable_spec()], Duration::from_secs(5), true)
            .await
            .expect("write mount"),
    );
    let provider = scripted_call("mcp__fixture__echo", json!({"text": "must not land"}));
    let mut log = SessionLog::create(directory.path()).unwrap();
    let mut config = AgentConfig::new("scripted", directory.path());
    config.provider = "scripted".into();
    config.approval = ApprovalMode::Ask;
    config.mcp_host = Some(host.clone());

    run_with_history(
        &provider,
        &mut log,
        &config,
        "call the write tool",
        Vec::new(),
        &mut |_request: &ApprovalRequest| false,
        &mut |_event| {},
    )
    .await
    .unwrap();

    let loaded = load(log.path()).unwrap();
    assert!(loaded.events.iter().any(|event| matches!(
        event,
        Event::Approval { tool, allowed: false, .. } if tool == "mcp__fixture__echo"
    )));
    let output = loaded
        .events
        .iter()
        .find_map(|event| match event {
            Event::ToolResult { output, .. } => Some(output.as_str()),
            _ => None,
        })
        .expect("denial result");
    assert_eq!(output, "approval denied by user");
    assert!(!loaded.events.iter().any(
        |event| matches!(event, Event::ToolResult { output, .. } if output == "must not land")
    ));
    host.shutdown().await.unwrap();
}

#[tokio::test]
async fn mutating_mcp_refuses_never_approval() {
    let directory = tempfile::tempdir().unwrap();
    let host = Arc::new(
        McpHost::mount_with(&[writable_spec()], Duration::from_secs(5), true)
            .await
            .expect("write mount"),
    );
    let provider = scripted_call("mcp__fixture__echo", json!({"text": "no approval"}));
    let mut log = SessionLog::create(directory.path()).unwrap();
    let mut config = AgentConfig::new("scripted", directory.path());
    config.provider = "scripted".into();
    config.approval = ApprovalMode::Never;
    config.mcp_host = Some(host.clone());

    run_with_history(
        &provider,
        &mut log,
        &config,
        "call the write tool",
        Vec::new(),
        &mut |_request: &ApprovalRequest| true,
        &mut |_event| {},
    )
    .await
    .unwrap();

    let loaded = load(log.path()).unwrap();
    let output = loaded
        .events
        .iter()
        .find_map(|event| match event {
            Event::ToolResult { output, .. } => Some(output.as_str()),
            _ => None,
        })
        .expect("refusal result");
    assert!(output.contains("interactive approval"), "{output}");
    assert!(!loaded
        .events
        .iter()
        .any(|event| matches!(event, Event::Approval { .. })));
    host.shutdown().await.unwrap();
}
