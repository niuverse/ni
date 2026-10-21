//! One session, two runs, two working directories.
//!
//! niuse's thread model relies on `working_dir` being read per run rather than
//! captured when the session starts. Pin that behaviour here so a refactor in
//! the loop cannot silently break it.

use std::fs;

use niucode_agent::{
    project_history, run_with_history, AgentConfig, AgentEvent, ApprovalMode, ApprovalRequest,
};
use niucode_core::{load, now_ms, Event, SessionLog};
use niucode_provider::{ScriptedProvider, StreamEvent, ToolCallDelta};
use niucode_tools::{NetworkMode, SandboxMode, SandboxPolicy};

fn config(directory: &std::path::Path) -> AgentConfig {
    let mut config = AgentConfig::new("scripted", directory);
    config.provider = "scripted".into();
    config.system_prompt = "cwd regression".into();
    config.sandbox = SandboxPolicy::new(SandboxMode::ReadOnly, NetworkMode::Off);
    config.approval = ApprovalMode::Never;
    config
}

fn tool_turn(call_id: &str, path: &str) -> Vec<StreamEvent> {
    vec![
        StreamEvent::ToolCallDelta(ToolCallDelta {
            index: 0,
            id: Some(call_id.into()),
            name: Some("read".into()),
            arguments: format!(r#"{{"path":"{path}"}}"#),
        }),
        StreamEvent::Done {
            finish_reason: Some("tool_calls".into()),
        },
    ]
}

fn text_turn(text: &str) -> Vec<StreamEvent> {
    vec![
        StreamEvent::TextDelta(text.into()),
        StreamEvent::Done {
            finish_reason: Some("stop".into()),
        },
    ]
}

#[tokio::test]
async fn working_directory_is_read_per_run_on_one_session() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    fs::write(first.path().join("a.txt"), "alpha").unwrap();
    fs::write(second.path().join("b.txt"), "beta").unwrap();

    let provider = ScriptedProvider::from_turns(vec![
        tool_turn("call-a", "a.txt"),
        text_turn("first done"),
        tool_turn("call-b", "b.txt"),
        text_turn("second done"),
    ]);

    let mut log = SessionLog::create(first.path()).unwrap();
    log.append(&Event::SessionStart {
        id: log.id().to_string(),
        cwd: first.path().display().to_string(),
        model: "scripted".into(),
        provider: "scripted".into(),
        ts: now_ms(),
    })
    .unwrap();

    fn deny(_request: &ApprovalRequest) -> bool {
        false
    }
    fn ignore(_event: AgentEvent) {}

    let first_outcome = run_with_history(
        &provider,
        &mut log,
        &config(first.path()),
        "read a",
        Vec::new(),
        &mut deny,
        &mut ignore,
    )
    .await
    .unwrap();
    assert_eq!(first_outcome.final_text, "first done");

    let history = project_history(&load(log.path()).unwrap().events).unwrap();
    let second_outcome = run_with_history(
        &provider,
        &mut log,
        &config(second.path()),
        "read b",
        history,
        &mut deny,
        &mut ignore,
    )
    .await
    .unwrap();
    assert_eq!(second_outcome.final_text, "second done");

    let events = load(log.path()).unwrap().events;
    let outputs: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            Event::ToolResult { output, .. } => Some(output.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        outputs.iter().any(|output| output.contains("alpha")),
        "first run should resolve against the first cwd: {outputs:?}"
    );
    assert!(
        outputs.iter().any(|output| output.contains("beta")),
        "second run should resolve against the second cwd: {outputs:?}"
    );
}
