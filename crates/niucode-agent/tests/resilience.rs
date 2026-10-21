use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use niucode_agent::{run, AgentConfig};
use niucode_core::SessionLog;
use niucode_provider::{ProviderError, ScriptedProvider, StreamEvent};
use niucode_tools::{NetworkMode, SandboxMode, SandboxPolicy};

fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("niucode-resilience-{tag}-{stamp}-{sequence}"));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create temp dir");
    directory
}

fn config(directory: &Path) -> AgentConfig {
    let mut config = AgentConfig::new("test-model", directory);
    config.sandbox = SandboxPolicy::new(SandboxMode::FullAccess, NetworkMode::On);
    config.approval = niucode_agent::ApprovalMode::Never;
    config.allow_unsandboxed = true;
    config.max_steps = 0;
    config
}

fn start_session(directory: &Path) -> SessionLog {
    let session = SessionLog::create(directory).expect("session");
    session
        .append(&niucode_core::Event::SessionStart {
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
async fn retries_a_transient_error_before_any_event_is_emitted() {
    let directory = temp_dir("retry");
    let provider = ScriptedProvider::from_results(vec![
        vec![Err(ProviderError::Decode("temporary truncated SSE".into()))],
        vec![
            Ok(StreamEvent::TextDelta("recovered".into())),
            Ok(StreamEvent::Done {
                finish_reason: Some("stop".into()),
            }),
        ],
    ]);
    let mut session = start_session(&directory);
    let config = config(&directory);

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        run(
            &provider,
            &mut session,
            &config,
            "recover",
            &mut |_| true,
            &mut |_| {},
        ),
    )
    .await
    .expect("retry timeout")
    .expect("retry should succeed");

    assert_eq!(outcome.final_text, "recovered");
    assert_eq!(provider.requests().len(), 2);
    assert_eq!(outcome.turns, 1);

    fs::remove_dir_all(directory).expect("cleanup");
}

#[tokio::test]
async fn does_not_replay_a_stream_after_partial_output() {
    let directory = temp_dir("partial");
    let provider = ScriptedProvider::from_results(vec![vec![
        Ok(StreamEvent::TextDelta("partial".into())),
        Err(ProviderError::Network("dropped".into())),
    ]]);
    let mut session = start_session(&directory);
    let config = config(&directory);

    let error = tokio::time::timeout(
        Duration::from_secs(5),
        run(
            &provider,
            &mut session,
            &config,
            "partial",
            &mut |_| true,
            &mut |_| {},
        ),
    )
    .await
    .expect("timeout")
    .expect_err("partial stream must fail");

    assert!(error.to_string().contains("network error"));
    assert_eq!(provider.requests().len(), 1);

    fs::remove_dir_all(directory).expect("cleanup");
}

#[tokio::test]
async fn generation_limit_never_completes_or_dispatches_truncated_tools() {
    use niucode_agent::{AgentEvent, LoopPhase};
    use niucode_core::{load, Event, Usage};
    use niucode_provider::ToolCallDelta;

    for arguments in [
        None,
        Some("{\"path\":"),
        Some(r#"{"path":"unexpected","content":"bad"}"#),
    ] {
        let directory = temp_dir("length");
        let mut events = vec![StreamEvent::ReasoningDelta("still thinking".into())];
        if let Some(arguments) = arguments {
            events.push(StreamEvent::TextDelta("partial answer".into()));
            events.push(StreamEvent::ToolCallDelta(ToolCallDelta {
                index: 0,
                id: Some("write-1".into()),
                name: Some("write".into()),
                arguments: arguments.into(),
            }));
        }
        events.push(StreamEvent::Done {
            finish_reason: Some("length".into()),
        });
        // Usage may arrive after the finish reason; consume the whole stream.
        events.push(StreamEvent::Usage(Usage {
            output_tokens: 8192,
            reasoning_tokens: 8192,
            ..Usage::default()
        }));
        let provider = ScriptedProvider::new(events);
        let mut session = start_session(&directory);
        let mut observed = Vec::new();
        let error = run(
            &provider,
            &mut session,
            &config(&directory),
            "implement",
            &mut |_| true,
            &mut |event| observed.push(event),
        )
        .await
        .expect_err("length must fail");
        assert!(error.to_string().contains("length"), "{error}");
        assert_eq!(provider.requests().len(), 1, "no automatic retry");
        assert!(!directory.join("unexpected").exists());
        assert!(!observed.iter().any(|e| matches!(
            e,
            AgentEvent::ToolCall { .. } | AgentEvent::Phase(LoopPhase::Done)
        )));
        let log = load(session.path()).unwrap().events;
        assert!(
            matches!(log.last(), Some(Event::TaskFinished { status, .. }) if status == "error")
        );
        assert!(log.iter().any(|e| matches!(e, Event::TurnEnd { usage, finish_reason: Some(reason), .. } if usage.output_tokens == 8192 && reason == "length")));
        assert!(log.iter().any(|e| matches!(e, Event::AssistantMessage { reasoning: Some(reasoning), tool_calls, .. } if reasoning == "still thinking" && tool_calls.is_empty())));
        assert!(!log
            .iter()
            .any(|e| matches!(e, Event::SessionEnd { .. } | Event::ToolResult { .. })));
        fs::remove_dir_all(directory).unwrap();
    }
}
