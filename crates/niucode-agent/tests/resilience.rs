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
    let mut session = SessionLog::create(directory).expect("session");
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
