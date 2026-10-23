use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use niucode_agent::{
    pending_inputs, run_interactive, runtime_channel, AgentConfig, ApprovalMode, Cancelled,
    RuntimeCommand, RuntimeEvent,
};
use niucode_core::{load, Event, SessionLog};
use niucode_provider::{
    CompletionRequest, EventStream, Provider, ProviderError, ScriptedProvider, StreamEvent,
    ToolCallDelta,
};
use tokio::sync::Notify;

struct GatedProvider {
    entered: Notify,
    release: Notify,
    first: AtomicBool,
    scripted: ScriptedProvider,
}
#[async_trait::async_trait]
impl Provider for GatedProvider {
    fn name(&self) -> &str {
        "gated"
    }
    async fn stream(&self, request: CompletionRequest) -> Result<EventStream, ProviderError> {
        if !self.first.swap(true, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        self.scripted.stream(request).await
    }
}

#[tokio::test]
async fn accepts_during_provider_wait_deduplicates_and_orders_queued_tasks() {
    let directory = tempfile::tempdir().unwrap();
    let log = SessionLog::create(directory.path()).unwrap();
    let provider = GatedProvider {
        entered: Notify::new(),
        release: Notify::new(),
        first: AtomicBool::new(false),
        scripted: ScriptedProvider::from_turns(
            ["first", "steered", "queued"]
                .into_iter()
                .map(|s| vec![StreamEvent::TextDelta(s.into())])
                .collect(),
        ),
    };
    let config = AgentConfig::new("test", directory.path().to_path_buf());
    let (mut client, endpoint) = runtime_channel();
    let driver = async {
        provider.entered.notified().await;
        for (id, text, queued) in [
            ("steer", "adjust", false),
            ("steer", "adjust", false),
            ("queue", "next task", true),
        ] {
            client
                .commands
                .send(RuntimeCommand::Submit {
                    id: id.into(),
                    text: text.into(),
                    queued,
                })
                .await
                .unwrap();
        }
        let mut acknowledged = 0;
        let mut done = 0;
        while let Some(event) = client.events.recv().await {
            match event {
                RuntimeEvent::Input { applied: false, .. } => {
                    acknowledged += 1;
                    // Acknowledgment implies a flushed event, before request completion.
                    assert!(!pending_inputs(&load(log.path()).unwrap().events).is_empty());
                    if acknowledged == 3 {
                        provider.release.notify_one();
                    }
                }
                RuntimeEvent::TaskDone { .. } => done += 1,
                _ => {}
            }
        }
        assert_eq!(done, 2);
    };
    let run = run_interactive(&provider, &log, &config, "original", vec![], endpoint);
    let (result, ()) =
        tokio::time::timeout(Duration::from_secs(5), async { tokio::join!(run, driver) })
            .await
            .unwrap();
    assert_eq!(result.unwrap().final_text, "queued");
    let events = load(log.path()).unwrap().events;
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::InputSubmitted { .. }))
            .count(),
        2
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::InputApplied { .. }))
            .count(),
        2
    );
    assert!(pending_inputs(&events).is_empty());
    let requests = provider.scripted.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[1].messages.last().unwrap().text(), Some("adjust"));
    assert_eq!(
        requests[2].messages.last().unwrap().text(),
        Some("next task")
    );
}

#[tokio::test]
async fn approval_wait_releases_write_lock_rejects_stale_reply_and_cancels() {
    let directory = tempfile::tempdir().unwrap();
    let log = SessionLog::create(directory.path()).unwrap();
    let provider = ScriptedProvider::new(vec![StreamEvent::ToolCallDelta(ToolCallDelta {
        index: 0,
        id: Some("write-1".into()),
        name: Some("write".into()),
        arguments: r#"{"path":"new.txt","content":"hello"}"#.into(),
    })]);
    let mut config = AgentConfig::new("test", directory.path().to_path_buf());
    config.approval = ApprovalMode::Ask;
    config.write_lock = Some(std::sync::Arc::new(tokio::sync::Mutex::new(())));
    let (mut client, endpoint) = runtime_channel();
    let driver = async {
        while let Some(event) = client.events.recv().await {
            match event {
                RuntimeEvent::Approval { call_id, .. } => {
                    assert_eq!(call_id, "write-1");
                    assert!(config.write_lock.as_ref().unwrap().try_lock().is_ok());
                    client
                        .commands
                        .send(RuntimeCommand::Approve {
                            request_id: "stale".into(),
                            allowed: true,
                        })
                        .await
                        .unwrap();
                }
                RuntimeEvent::Notice(_) => {
                    assert!(!directory.path().join("new.txt").exists());
                    client
                        .commands
                        .send(RuntimeCommand::Submit {
                            id: "pending".into(),
                            text: "later".into(),
                            queued: true,
                        })
                        .await
                        .unwrap();
                }
                RuntimeEvent::Input { .. } => client.cancel(),
                _ => {}
            }
        }
    };
    let run = run_interactive(&provider, &log, &config, "write", vec![], endpoint);
    let (result, ()) =
        tokio::time::timeout(Duration::from_secs(5), async { tokio::join!(run, driver) })
            .await
            .unwrap();
    assert!(result.unwrap_err().is::<Cancelled>());
    let events = load(log.path()).unwrap().events;
    assert_eq!(pending_inputs(&events)[0].id, "pending");
    assert!(
        matches!(events.last(), Some(Event::TaskFinished { status, .. }) if status == "cancelled")
    );
    assert!(!directory.path().join("new.txt").exists());
}

#[tokio::test]
async fn disconnect_drops_waiting_provider_without_replaying_effects() {
    let directory = tempfile::tempdir().unwrap();
    let log = SessionLog::create(directory.path()).unwrap();
    let provider = GatedProvider {
        entered: Notify::new(),
        release: Notify::new(),
        first: AtomicBool::new(false),
        scripted: ScriptedProvider::new(vec![]),
    };
    let config = AgentConfig::new("test", directory.path().to_path_buf());
    let (client, endpoint) = runtime_channel();
    let driver = async {
        provider.entered.notified().await;
        drop(client);
    };
    let run = run_interactive(&provider, &log, &config, "wait", vec![], endpoint);
    let (result, ()) =
        tokio::time::timeout(Duration::from_secs(5), async { tokio::join!(run, driver) })
            .await
            .unwrap();
    assert!(result.unwrap_err().is::<Cancelled>());
    assert!(provider.scripted.requests().is_empty());
}

#[tokio::test]
async fn truncated_response_never_notifies_interactive_client_of_success() {
    let directory = tempfile::tempdir().unwrap();
    let log = SessionLog::create(directory.path()).unwrap();
    let provider = ScriptedProvider::new(vec![
        StreamEvent::ReasoningDelta("unfinished reasoning".into()),
        StreamEvent::Done {
            finish_reason: Some("length".into()),
        },
    ]);
    let config = AgentConfig::new("test", directory.path());
    let (mut client, endpoint) = runtime_channel();
    let driver = async {
        while let Some(event) = client.events.recv().await {
            assert!(!matches!(event, RuntimeEvent::TaskDone { .. }));
        }
    };
    let run = run_interactive(&provider, &log, &config, "implement", vec![], endpoint);
    let (result, ()) =
        tokio::time::timeout(Duration::from_secs(5), async { tokio::join!(run, driver) })
            .await
            .unwrap();
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("finish_reason=length"));
    assert!(
        matches!(load(log.path()).unwrap().events.last(), Some(Event::TaskFinished { status, .. }) if status == "error")
    );
}
