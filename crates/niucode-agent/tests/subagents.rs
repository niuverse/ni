use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures_util::stream;
use niucode_agent::{run, AgentConfig, ApprovalMode};
use niucode_core::{load, Event, SessionLog};
use niucode_provider::{
    CompletionRequest, EventStream, Provider, ProviderError, Role, StreamEvent, ToolCallDelta,
};
use niucode_tools::{NetworkMode, SandboxMode, SandboxPolicy};
use serde_json::json;

fn temp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::AtomicU64;
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("niucode-subagent-{tag}-{stamp}-{sequence}"));
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

fn subagent_call(tasks: serde_json::Value) -> Vec<Result<StreamEvent, ProviderError>> {
    vec![
        Ok(StreamEvent::ToolCallDelta(ToolCallDelta {
            index: 0,
            id: Some("call_1".into()),
            name: Some("subagent".into()),
            arguments: json!({"tasks": tasks}).to_string(),
        })),
        Ok(StreamEvent::Done {
            finish_reason: Some("tool_calls".into()),
        }),
    ]
}

fn text(text: &str) -> Vec<Result<StreamEvent, ProviderError>> {
    vec![
        Ok(StreamEvent::TextDelta(text.to_string())),
        Ok(StreamEvent::Done {
            finish_reason: Some("stop".into()),
        }),
    ]
}

struct ParallelProvider {
    calls: AtomicUsize,
    barrier: Arc<tokio::sync::Barrier>,
    requests: Mutex<Vec<CompletionRequest>>,
}

impl ParallelProvider {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            barrier: Arc::new(tokio::sync::Barrier::new(2)),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<CompletionRequest> {
        self.requests.lock().expect("requests").clone()
    }
}

#[async_trait]
impl Provider for ParallelProvider {
    fn name(&self) -> &str {
        "parallel-test"
    }

    async fn stream(&self, request: CompletionRequest) -> Result<EventStream, ProviderError> {
        self.requests.lock().expect("requests").push(request);
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        match call {
            0 => Ok(Box::pin(stream::iter(subagent_call(json!([
                {"prompt": "inspect a"},
                {"prompt": "inspect b"}
            ]))))),
            1 | 2 => {
                // If the runner is sequential, the first subagent blocks here
                // and the test's outer timeout fails.
                self.barrier.wait().await;
                let sub = call - 1;
                Ok(Box::pin(stream::iter(text(&format!(
                    "summary for task {sub}"
                )))))
            }
            _ => Ok(Box::pin(stream::iter(text("parent done")))),
        }
    }
}

#[tokio::test]
async fn two_subagents_run_in_parallel_with_independent_logs() {
    let directory = temp_dir("parallel");
    // Snapshots legitimately contain workspace text, including other task names.
    fs::write(directory.join("notes.txt"), "inspect a\ninspect b\n").expect("notes");
    let provider = ParallelProvider::new();
    let mut session = start_session(&directory);
    let config = config(&directory);

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        run(
            &provider,
            &mut session,
            &config,
            "delegate the exploration",
            &mut |_| true,
            &mut |_| {},
        ),
    )
    .await
    .expect("subagents must run in parallel")
    .expect("run");
    assert_eq!(outcome.final_text, "parent done");

    let loaded = load(session.path()).expect("load parent");
    let spawns = loaded
        .events
        .iter()
        .filter_map(|event| match event {
            Event::SubagentSpawn {
                id, session, task, ..
            } => Some((id.clone(), session.clone(), task.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    let results = loaded
        .events
        .iter()
        .filter_map(|event| match event {
            Event::SubagentResult {
                id, ok, summary, ..
            } => Some((id.clone(), *ok, summary.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(spawns.len(), 2);
    assert_eq!(results.len(), 2);
    assert_ne!(spawns[0].0, spawns[1].0);
    assert_ne!(spawns[0].1, spawns[1].1);
    assert!(results.iter().all(|(_, ok, _)| *ok));

    for (id, path, task) in &spawns {
        let sub = load(Path::new(path)).expect("load subagent");
        assert!(matches!(
            sub.events.first(),
            Some(Event::SessionStart { id: sub_id, .. }) if sub_id == id
        ));
        let prompts: Vec<_> = sub
            .events
            .iter()
            .filter_map(|event| match event {
                Event::UserMessage { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            prompts,
            [task.as_str()],
            "subagent prompts must not be mixed"
        );
        let answers: Vec<_> = sub
            .events
            .iter()
            .filter_map(|event| match event {
                Event::AssistantMessage { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        let result = results
            .iter()
            .find(|result| &result.0 == id)
            .expect("matching result");
        assert_eq!(
            answers,
            [result.2.as_str()],
            "subagent results must not be mixed"
        );
    }

    let requests = provider.requests();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0].tools.len(), 7, "parent sees all tools");
    for request in &requests[1..3] {
        let mut names = request
            .tools
            .iter()
            .map(|tool| tool.function.name.as_str())
            .collect::<Vec<_>>();
        names.sort_unstable();
        assert_eq!(names, ["glob", "grep", "read"]);
    }

    let final_request = &requests[3];
    assert_eq!(final_request.messages.len(), 4);
    assert_eq!(final_request.messages[3].role, Role::Tool);
    let result = final_request.messages[3]
        .content
        .as_deref()
        .expect("tool result");
    assert!(result.starts_with("subagent: 2 task(s)"));
    for (_, path, _) in &spawns {
        assert!(result.contains(path));
    }

    fs::remove_dir_all(directory).expect("cleanup");
}

struct TimeoutProvider {
    calls: AtomicUsize,
    requests: Mutex<Vec<CompletionRequest>>,
}

#[async_trait]
impl Provider for TimeoutProvider {
    fn name(&self) -> &str {
        "timeout-test"
    }

    async fn stream(&self, request: CompletionRequest) -> Result<EventStream, ProviderError> {
        self.requests.lock().expect("requests").push(request);
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        match call {
            0 => Ok(Box::pin(stream::iter(subagent_call(json!([
                {"prompt": "slow task", "timeout_secs": 1, "max_steps": 2}
            ]))))),
            1 => {
                tokio::time::sleep(Duration::from_secs(5)).await;
                Ok(Box::pin(stream::iter(text("too late"))))
            }
            _ => Ok(Box::pin(stream::iter(text("parent continued")))),
        }
    }
}

#[tokio::test]
async fn a_timed_out_subagent_does_not_stop_the_parent() {
    let directory = temp_dir("timeout");
    let provider = TimeoutProvider {
        calls: AtomicUsize::new(0),
        requests: Mutex::new(Vec::new()),
    };
    let mut session = start_session(&directory);
    let config = config(&directory);

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        run(
            &provider,
            &mut session,
            &config,
            "delegate",
            &mut |_| true,
            &mut |_| {},
        ),
    )
    .await
    .expect("timeout must be enforced")
    .expect("run");
    assert_eq!(outcome.final_text, "parent continued");

    let loaded = load(session.path()).expect("load");
    let spawn = loaded
        .events
        .iter()
        .find_map(|event| match event {
            Event::SubagentSpawn { session, .. } => Some(session.clone()),
            _ => None,
        })
        .expect("spawn");
    let result = loaded
        .events
        .iter()
        .find_map(|event| match event {
            Event::SubagentResult { ok, summary, .. } => Some((*ok, summary.clone())),
            _ => None,
        })
        .expect("result");
    assert!(!result.0);
    assert!(result.1.contains("timeout after 1s"));

    let sub = load(Path::new(&spawn)).expect("load subagent");
    assert!(matches!(sub.events.last(), Some(Event::SessionEnd { .. })));

    fs::remove_dir_all(directory).expect("cleanup");
}

struct ErrorProvider {
    calls: AtomicUsize,
}

#[async_trait]
impl Provider for ErrorProvider {
    fn name(&self) -> &str {
        "error-test"
    }

    async fn stream(&self, _request: CompletionRequest) -> Result<EventStream, ProviderError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        match call {
            0 => Ok(Box::pin(stream::iter(subagent_call(json!([
                {"prompt": "failing task"}
            ]))))),
            1..=4 => Err(ProviderError::Network("boom".into())),
            _ => Ok(Box::pin(stream::iter(text("parent continued")))),
        }
    }
}

#[tokio::test]
async fn a_failed_subagent_does_not_stop_the_parent() {
    let directory = temp_dir("error");
    let provider = ErrorProvider {
        calls: AtomicUsize::new(0),
    };
    let mut session = start_session(&directory);
    let config = config(&directory);

    let outcome = run(
        &provider,
        &mut session,
        &config,
        "delegate",
        &mut |_| true,
        &mut |_| {},
    )
    .await
    .expect("run");
    assert_eq!(outcome.final_text, "parent continued");

    let loaded = load(session.path()).expect("load");
    let result = loaded
        .events
        .iter()
        .find_map(|event| match event {
            Event::SubagentResult { ok, summary, .. } => Some((*ok, summary.clone())),
            _ => None,
        })
        .expect("result");
    assert!(!result.0);
    assert!(result.1.contains("boom"));

    fs::remove_dir_all(directory).expect("cleanup");
}

struct WorkerProvider {
    calls: AtomicUsize,
    requests: Mutex<Vec<CompletionRequest>>,
}

#[async_trait]
impl Provider for WorkerProvider {
    fn name(&self) -> &str {
        "worker-test"
    }

    async fn stream(&self, request: CompletionRequest) -> Result<EventStream, ProviderError> {
        self.requests.lock().expect("requests").push(request);
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        match call {
            0 => Ok(Box::pin(stream::iter(subagent_call(json!([
                {"prompt": "make a small edit", "profile": "worker", "write_scope": ["src"]}
            ]))))),
            1 => Ok(Box::pin(stream::iter(text("worker done")))),
            _ => Ok(Box::pin(stream::iter(text("parent done")))),
        }
    }
}

#[tokio::test]
async fn worker_profile_advertises_write_tools_and_records_profile() {
    let directory = temp_dir("worker");
    let provider = WorkerProvider {
        calls: AtomicUsize::new(0),
        requests: Mutex::new(Vec::new()),
    };
    let mut session = start_session(&directory);
    let config = config(&directory);
    let mut config = config;
    config.write_lock = Some(Arc::new(tokio::sync::Mutex::new(())));

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        run(
            &provider,
            &mut session,
            &config,
            "delegate a worker",
            &mut |_| true,
            &mut |_| {},
        ),
    )
    .await
    .expect("worker run")
    .expect("run");
    assert_eq!(outcome.final_text, "parent done");

    let loaded = load(session.path()).expect("load parent");
    assert!(loaded.events.iter().any(|event| matches!(
        event,
        Event::SubagentSpawn { profile, .. } if profile == "worker"
    )));

    let requests = provider.requests.lock().expect("requests");
    let subagent = &requests[1];
    let mut names = subagent
        .tools
        .iter()
        .map(|tool| tool.function.name.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(names, ["bash", "edit", "glob", "grep", "read", "write"]);

    fs::remove_dir_all(directory).expect("cleanup");
}

#[cfg(target_os = "linux")]
struct ExploreProvider {
    calls: AtomicUsize,
    requests: Mutex<Vec<CompletionRequest>>,
}

#[cfg(target_os = "linux")]
#[async_trait]
impl Provider for ExploreProvider {
    fn name(&self) -> &str {
        "explore-ext-test"
    }

    async fn stream(&self, request: CompletionRequest) -> Result<EventStream, ProviderError> {
        self.requests.lock().expect("requests").push(request);
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        match call {
            0 => Ok(Box::pin(stream::iter(subagent_call(json!([
                {"prompt": "explore with docs", "profile": "explore"}
            ]))))),
            1 => Ok(Box::pin(stream::iter(text("explored")))),
            _ => Ok(Box::pin(stream::iter(text("parent done")))),
        }
    }
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn explore_profile_can_use_read_only_external_tools() {
    let directory = temp_dir("explore-ext");
    std::fs::create_dir_all(directory.join(".niucode/extensions")).expect("extensions");
    std::fs::write(
        directory.join(".niucode/extensions/demo.toml"),
        "name = \"demo\"\ncommand = \"python3\"\nargs = [\"sidecar.py\"]\n",
    )
    .expect("manifest");
    std::fs::write(
        directory.join("sidecar.py"),
        r#"import json, sys
for line in sys.stdin:
    request = json.loads(line); method = request.get("method")
    if method == "initialize":
        result = {"name": "demo", "version": "0.1.0", "tools": [{
            "name": "demo_echo",
            "description": "Echo text.",
            "input_schema": {"type": "object", "properties": {"text": {"type": "string"}}}
        }]}
    elif method == "tools/call":
        text = request.get("params", {}).get("arguments", {}).get("text", "")
        result = {"ok": True, "output": "echo: " + text}
    elif method == "shutdown":
        sys.exit(0)
    else:
        result = {"ok": False, "output": "unexpected"}
    print(json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "result": result}), flush=True)
"#,
    )
    .expect("sidecar");
    let reserved = niucode_tools::Toolbox::specs()
        .into_iter()
        .map(|tool| tool.function.name)
        .collect::<Vec<_>>();
    let report = niucode_extension::discover(&directory, &reserved).await;
    assert!(report.errors.is_empty(), "{:?}", report.errors);

    let provider = ExploreProvider {
        calls: AtomicUsize::new(0),
        requests: Mutex::new(Vec::new()),
    };
    let mut session = start_session(&directory);
    let mut config = config(&directory);
    config.extension_host = Some(Arc::new(report.host));

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        run(
            &provider,
            &mut session,
            &config,
            "delegate explore",
            &mut |_| true,
            &mut |_| {},
        ),
    )
    .await
    .expect("explore run")
    .expect("run");
    assert_eq!(outcome.final_text, "parent done");

    {
        let requests = provider.requests.lock().expect("requests");
        let subagent = &requests[1];
        let mut names = subagent
            .tools
            .iter()
            .map(|tool| tool.function.name.as_str())
            .collect::<Vec<_>>();
        names.sort_unstable();
        assert_eq!(names, ["demo_echo", "glob", "grep", "read"]);
    }

    if let Some(host) = &config.extension_host {
        host.shutdown().await;
    }
    std::fs::remove_dir_all(directory).expect("cleanup");
}

struct InheritProvider {
    calls: AtomicUsize,
    requests: Mutex<Vec<CompletionRequest>>,
}

#[async_trait]
impl Provider for InheritProvider {
    fn name(&self) -> &str {
        "inherit-test"
    }

    async fn stream(&self, request: CompletionRequest) -> Result<EventStream, ProviderError> {
        self.requests.lock().expect("requests").push(request);
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        match call {
            0 => Ok(Box::pin(stream::iter(subagent_call(json!([
                {"prompt": "inherit the tools", "profile": "inherit"}
            ]))))),
            1 => Ok(Box::pin(stream::iter(text("inherited")))),
            _ => Ok(Box::pin(stream::iter(text("parent done")))),
        }
    }
}

#[tokio::test]
async fn inherit_profile_requires_explicit_opt_in() {
    let directory = temp_dir("inherit-guard");
    let provider = InheritProvider {
        calls: AtomicUsize::new(0),
        requests: Mutex::new(Vec::new()),
    };
    let mut session = start_session(&directory);
    let config = config(&directory); // allow_subagent_inheritance defaults false

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        run(
            &provider,
            &mut session,
            &config,
            "try inherit",
            &mut |_| true,
            &mut |_| {},
        ),
    )
    .await
    .expect("run")
    .expect("run");
    assert_eq!(outcome.final_text, "inherited");

    let loaded = load(session.path()).expect("load");
    assert!(loaded.events.iter().any(|event| matches!(
        event,
        Event::ToolResult { ok: false, output, .. }
            if output.contains("allow-subagent-inheritance")
    )));

    fs::remove_dir_all(directory).expect("cleanup");
}

#[tokio::test]
async fn inherit_profile_copies_the_parent_surface_but_not_nested_subagents() {
    let directory = temp_dir("inherit-ok");
    let provider = InheritProvider {
        calls: AtomicUsize::new(0),
        requests: Mutex::new(Vec::new()),
    };
    let mut session = start_session(&directory);
    let mut config = config(&directory);
    config.allow_subagent_inheritance = true;
    config.write_lock = Some(Arc::new(tokio::sync::Mutex::new(())));

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        run(
            &provider,
            &mut session,
            &config,
            "inherit",
            &mut |_| true,
            &mut |_| {},
        ),
    )
    .await
    .expect("run")
    .expect("run");
    assert_eq!(outcome.final_text, "parent done");

    {
        let requests = provider.requests.lock().expect("requests");
        let subagent = &requests[1];
        let mut names = subagent
            .tools
            .iter()
            .map(|tool| tool.function.name.as_str())
            .collect::<Vec<_>>();
        names.sort_unstable();
        assert_eq!(names, ["bash", "edit", "glob", "grep", "read", "write"]);
    }

    let loaded = load(session.path()).expect("load");
    assert!(loaded.events.iter().any(|event| matches!(
        event,
        Event::SubagentSpawn { profile, .. } if profile == "inherit"
    )));

    fs::remove_dir_all(directory).expect("cleanup");
}
