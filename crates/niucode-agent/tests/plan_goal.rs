use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;
use futures_util::stream;
use niucode_agent::{run, AgentConfig, ApprovalMode};
use niucode_core::{now_ms, Event, SessionLog};
use niucode_provider::{
    CompletionRequest, EventStream, Provider, ProviderError, Role, StreamEvent, ToolCallDelta,
};
use niucode_tools::{NetworkMode, SandboxMode, SandboxPolicy};
use serde_json::json;

fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "niucode-plan-goal-{tag}-{stamp}-{}",
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("temp dir");
    directory
}

struct CaptureProvider {
    requests: Mutex<Vec<CompletionRequest>>,
}

#[async_trait]
impl Provider for CaptureProvider {
    fn name(&self) -> &str {
        "capture"
    }

    async fn stream(&self, request: CompletionRequest) -> Result<EventStream, ProviderError> {
        self.requests.lock().expect("requests").push(request);
        Ok(Box::pin(stream::iter(vec![
            Ok(StreamEvent::TextDelta("plan ready".into())),
            Ok(StreamEvent::Done {
                finish_reason: Some("stop".into()),
            }),
        ])))
    }
}

#[tokio::test]
async fn plan_mode_narrows_tools_and_injects_goal_and_directive() {
    let directory = temp_dir("m4");
    let provider = CaptureProvider {
        requests: Mutex::new(Vec::new()),
    };
    let mut session = SessionLog::create(&directory).expect("session");
    session
        .append(&Event::SessionStart {
            id: session.id().to_string(),
            cwd: directory.display().to_string(),
            model: "capture".into(),
            provider: "deepseek".into(),
            ts: now_ms(),
        })
        .expect("start");

    let mut config = AgentConfig::new("capture", &directory);
    config.sandbox = SandboxPolicy::new(SandboxMode::FullAccess, NetworkMode::On);
    config.approval = ApprovalMode::Never;
    config.allow_unsandboxed = true;
    config.max_steps = 1;
    config.plan_mode = true;
    config.goal = Some("all workspace tests pass".into());
    config.allowed_tools = Some(vec!["read".into(), "grep".into(), "glob".into()]);

    let outcome = run(
        &provider,
        &mut session,
        &config,
        "find the failing test",
        &mut |_| true,
        &mut |_| {},
    )
    .await
    .expect("run");
    assert_eq!(outcome.final_text, "plan ready");

    let requests = provider.requests.lock().expect("requests");
    let request = requests.first().expect("request");
    let mut names = request
        .tools
        .iter()
        .map(|tool| tool.function.name.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(names, ["glob", "grep", "read"]);

    let system = request
        .messages
        .iter()
        .find(|message| message.role == Role::System)
        .and_then(|message| message.content.as_deref())
        .expect("system message");
    assert!(system.contains("Plan mode"));
    assert!(system.contains("Active goal"));
    assert!(system.contains("all workspace tests pass"));

    std::fs::remove_dir_all(directory).expect("cleanup");
}

struct PlanWriteProvider {
    calls: AtomicU64,
}

#[async_trait]
impl Provider for PlanWriteProvider {
    fn name(&self) -> &str {
        "plan-write"
    }

    async fn stream(&self, _request: CompletionRequest) -> Result<EventStream, ProviderError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            Ok(Box::pin(stream::iter(vec![
                Ok(StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 0,
                    id: Some("write_1".into()),
                    name: Some("write".into()),
                    arguments: json!({"path": "a.txt", "content": "nope"}).to_string(),
                })),
                Ok(StreamEvent::Done {
                    finish_reason: Some("tool_calls".into()),
                }),
            ])))
        } else {
            Ok(Box::pin(stream::iter(vec![
                Ok(StreamEvent::TextDelta("plan only".into())),
                Ok(StreamEvent::Done {
                    finish_reason: Some("stop".into()),
                }),
            ])))
        }
    }
}

#[tokio::test]
async fn plan_mode_blocks_a_write_call_even_if_the_model_asks_for_it() {
    let directory = temp_dir("blocked");
    let provider = PlanWriteProvider {
        calls: AtomicU64::new(0),
    };
    let mut session = SessionLog::create(&directory).expect("session");
    session
        .append(&Event::SessionStart {
            id: session.id().to_string(),
            cwd: directory.display().to_string(),
            model: "plan-write".into(),
            provider: "deepseek".into(),
            ts: now_ms(),
        })
        .expect("start");

    let mut config = AgentConfig::new("plan-write", &directory);
    config.sandbox = SandboxPolicy::new(SandboxMode::FullAccess, NetworkMode::On);
    config.approval = ApprovalMode::Never;
    config.allow_unsandboxed = true;
    config.max_steps = 2;
    config.plan_mode = true;
    config.allowed_tools = Some(vec!["read".into(), "grep".into(), "glob".into()]);

    run(
        &provider,
        &mut session,
        &config,
        "make a plan",
        &mut |_| true,
        &mut |_| {},
    )
    .await
    .expect("run");

    assert!(!directory.join("a.txt").exists());
    let loaded = niucode_core::load(session.path()).expect("load");
    assert!(loaded.events.iter().any(|event| matches!(
        event,
        Event::ToolResult { ok: false, output, .. } if output.contains("not available")
    )));

    std::fs::remove_dir_all(directory).expect("cleanup");
}
