//! The embedding surface niuse and other consumers rely on.
//!
//! These pin two behaviours that are easy to break while refactoring the loop:
//! the configured system prompt reaches the provider unchanged, and a resumed
//! run receives the projected history rather than an empty transcript.

use niucode_agent::{
    project_history, run_with_history, AgentConfig, AgentEvent, ApprovalMode, ApprovalRequest,
};
use niucode_core::{load, now_ms, Event, SessionLog};
use niucode_provider::{ScriptedProvider, StreamEvent};
use niucode_tools::{NetworkMode, SandboxMode, SandboxPolicy};

fn config(directory: &std::path::Path) -> AgentConfig {
    let mut config = AgentConfig::new("scripted", directory);
    config.provider = "scripted".into();
    config.system_prompt = "embedder prompt".into();
    config.sandbox = SandboxPolicy::new(SandboxMode::ReadOnly, NetworkMode::Off);
    config.approval = ApprovalMode::Never;
    config
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
async fn system_prompt_and_history_reach_the_provider() {
    let directory = tempfile::tempdir().unwrap();
    let provider = ScriptedProvider::from_turns(vec![text_turn("first"), text_turn("second")]);

    let mut log = SessionLog::create(directory.path()).unwrap();
    log.append(&Event::SessionStart {
        id: log.id().to_string(),
        cwd: directory.path().display().to_string(),
        model: "scripted".into(),
        provider: "scripted".into(),
        ts: now_ms(),
    })
    .unwrap();

    fn deny(_request: &ApprovalRequest) -> bool {
        false
    }
    fn ignore(_event: AgentEvent) {}

    run_with_history(
        &provider,
        &mut log,
        &config(directory.path()),
        "one",
        Vec::new(),
        &mut deny,
        &mut ignore,
    )
    .await
    .unwrap();

    let requests = provider.requests();
    assert_eq!(
        requests[0].messages[0].content.as_deref(),
        Some("embedder prompt"),
        "the configured system prompt must be sent unchanged"
    );

    let history = project_history(&load(log.path()).unwrap().events).unwrap();
    run_with_history(
        &provider,
        &mut log,
        &config(directory.path()),
        "two",
        history,
        &mut deny,
        &mut ignore,
    )
    .await
    .unwrap();

    let second = provider.requests().remove(1);
    let contents: Vec<&str> = second
        .messages
        .iter()
        .filter_map(|message| message.content.as_deref())
        .collect();
    assert!(contents.contains(&"one"), "previous user turn is missing");
    assert!(contents.contains(&"first"), "previous answer is missing");
    assert!(contents.contains(&"two"), "current turn is missing");
}
