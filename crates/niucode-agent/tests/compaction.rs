use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use niucode_agent::{
    project_history, run, run_with_history, AgentConfig, AgentEvent, ApprovalMode,
};
use niucode_core::{load, Event, SessionLog};
use niucode_provider::{ScriptedProvider, StreamEvent, ToolCallDelta};
use niucode_tools::{NetworkMode, SandboxMode, SandboxPolicy};
use serde_json::json;

fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("niucode-compact-{tag}-{stamp}-{sequence}"));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create temp dir");
    directory
}

fn config(directory: &Path) -> AgentConfig {
    let mut config = AgentConfig::new("test-model", directory.to_path_buf());
    config.sandbox = SandboxPolicy::new(SandboxMode::FullAccess, NetworkMode::On);
    config.approval = ApprovalMode::Never;
    config.allow_unsandboxed = true;
    config.max_steps = 4;
    config
}

fn start_session(directory: &Path) -> SessionLog {
    let session = SessionLog::create(directory).expect("session");
    session
        .append(&Event::SessionStart {
            id: session.id().to_string(),
            cwd: directory.display().to_string(),
            model: "test-model".into(),
            provider: "deepseek".into(),
            ts: 1,
        })
        .expect("session start");
    session
}

#[tokio::test]
async fn compaction_preserves_a_replayable_projection_and_changes() {
    let directory = temp_dir("replay");
    let content = format!("{}\n", "let value = 42;\n".repeat(800));
    let write_arguments = json!({"path": "a.txt", "content": content}).to_string();
    let provider = ScriptedProvider::from_turns(vec![
        vec![
            StreamEvent::ToolCallDelta(ToolCallDelta {
                index: 0,
                id: Some("write_1".into()),
                name: Some("write".into()),
                arguments: write_arguments,
            }),
            StreamEvent::Done {
                finish_reason: Some("tool_calls".into()),
            },
        ],
        vec![
            StreamEvent::TextDelta("wrote a.txt".into()),
            StreamEvent::Done {
                finish_reason: Some("stop".into()),
            },
        ],
        vec![
            StreamEvent::TextDelta("second done".into()),
            StreamEvent::Done {
                finish_reason: Some("stop".into()),
            },
        ],
    ]);
    let mut session = start_session(&directory);
    let first = config(&directory);
    run(
        &provider,
        &mut session,
        &first,
        "first request",
        &mut |_| true,
        &mut |_| {},
    )
    .await
    .expect("first run");
    let history = project_history(&load(session.path()).expect("load").events).expect("history");

    let mut second = config(&directory);
    second.context.budget_tokens = 2_000;
    second.context.keep_recent_turns = 1;
    let mut observed = Vec::new();
    run_with_history(
        &provider,
        &mut session,
        &second,
        "second request",
        history,
        &mut |_| true,
        &mut |event| observed.push(event),
    )
    .await
    .expect("second run");

    let loaded = load(session.path()).expect("load");
    let compactions = loaded
        .events
        .iter()
        .filter_map(|event| match event {
            Event::Compaction {
                summary,
                upto,
                before_tokens,
                after_tokens,
                ..
            } => Some((summary, *upto, *before_tokens, *after_tokens)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(compactions.len(), 1, "one compaction event");
    let (summary, upto, before_tokens, after_tokens) = compactions[0];
    assert_eq!(
        upto as usize,
        loaded
            .events
            .iter()
            .position(|e| matches!(e, Event::UserMessage { text, .. } if text == "second request"))
            .unwrap(),
        "cutoff is the second user message"
    );
    assert!(before_tokens > after_tokens);
    assert!(summary.contains("# Session handoff"));
    assert!(summary.contains("first request"));
    assert!(summary.contains("`write` a.txt"));
    assert!(summary.contains("## Changed files"));
    assert!(summary.contains("- a.txt"));
    assert!(!summary.contains("second request"));

    assert!(observed.iter().any(|event| matches!(
        event,
        AgentEvent::Compaction {
            replaced_events,
            before_tokens: before,
            after_tokens: after,
        } if before > after && *replaced_events == upto as usize
    )));

    let requests = provider.requests();
    let compacted = &requests[2];
    assert_eq!(compacted.messages[0].role, niucode_provider::Role::System);
    assert_eq!(compacted.messages[1].role, niucode_provider::Role::System);
    assert!(compacted.messages[1]
        .text()
        .is_some_and(|content| content.contains("# Session handoff")));
    assert_eq!(compacted.messages[2].text(), Some("second request"));
    assert!(
        compacted
            .messages
            .iter()
            .all(|message| message.text() != Some("first request")),
        "the summarized request must not also be projected raw"
    );

    let final_history = project_history(&loaded.events).expect("final history");
    assert_eq!(
        compacted.messages[1..],
        final_history[..compacted.messages.len() - 1],
        "the compaction event replays to the same projection"
    );

    fs::remove_dir_all(directory).expect("cleanup");
}

#[tokio::test]
async fn compaction_failure_keeps_the_original_context_and_warns() {
    let directory = temp_dir("failure");
    let provider = ScriptedProvider::from_turns(vec![vec![
        StreamEvent::TextDelta("done".into()),
        StreamEvent::Done {
            finish_reason: Some("stop".into()),
        },
    ]]);
    let mut session = start_session(&directory);
    let mut config = config(&directory);
    config.context.budget_tokens = 1;
    let mut observed = Vec::new();

    let outcome = run(
        &provider,
        &mut session,
        &config,
        "keep this request",
        &mut |_| true,
        &mut |event| observed.push(event),
    )
    .await
    .expect("run");

    assert_eq!(outcome.final_text, "done");
    assert!(observed.iter().any(|event| matches!(
        event,
        AgentEvent::Warning(message) if message.contains("compaction skipped")
    )));
    let requests = provider.requests();
    assert_eq!(
        requests[0]
            .messages
            .last()
            .and_then(|message| message.text()),
        Some("keep this request")
    );
    let loaded = load(session.path()).expect("load");
    assert!(!loaded
        .events
        .iter()
        .any(|event| matches!(event, Event::Compaction { .. })));

    fs::remove_dir_all(directory).expect("cleanup");
}
