// The external tool fixture starts a real sidecar through Linux/bubblewrap.
#![cfg(target_os = "linux")]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures_util::stream;
use niucode_agent::{run, AgentConfig, ApprovalMode};
use niucode_core::{load, now_ms, Event, SessionLog};
use niucode_extension::discover;
use niucode_provider::{
    CompletionRequest, EventStream, Provider, ProviderError, StreamEvent, ToolCallDelta,
};
use niucode_tools::{NetworkMode, SandboxMode, SandboxPolicy, Toolbox};
use serde_json::json;

fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "niucode-agent-ext-{tag}-{stamp}-{}",
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join(".niucode/extensions")).expect("dirs");
    dir
}

struct ExternalProvider {
    calls: AtomicUsize,
    requests: Mutex<Vec<CompletionRequest>>,
}

#[async_trait]
impl Provider for ExternalProvider {
    fn name(&self) -> &str {
        "external"
    }

    async fn stream(&self, request: CompletionRequest) -> Result<EventStream, ProviderError> {
        self.requests.lock().expect("requests").push(request);
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            Ok(Box::pin(stream::iter(vec![
                Ok(StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 0,
                    id: Some("ext_1".into()),
                    name: Some("demo_echo".into()),
                    arguments: json!({"text": "hello"}).to_string(),
                })),
                Ok(StreamEvent::Done {
                    finish_reason: Some("tool_calls".into()),
                }),
            ])))
        } else {
            Ok(Box::pin(stream::iter(vec![
                Ok(StreamEvent::TextDelta("done".into())),
                Ok(StreamEvent::Done {
                    finish_reason: Some("stop".into()),
                }),
            ])))
        }
    }
}

#[tokio::test]
async fn the_loop_advertises_and_calls_an_external_tool() {
    let dir = temp_dir("loop");
    fs::write(
        dir.join(".niucode/extensions/demo.toml"),
        r#"
name = "demo"
command = "python3"
args = ["sidecar.py"]
timeout_secs = 10
"#,
    )
    .expect("manifest");
    fs::write(
        dir.join("sidecar.py"),
        r#"import json, sys
for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "initialize":
        result = {"name": "demo", "version": "0.1.0", "tools": [{
            "name": "demo_echo",
            "description": "Echo text from the demo extension.",
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

    let reserved = Toolbox::specs()
        .into_iter()
        .map(|tool| tool.function.name)
        .collect::<Vec<_>>();
    let report = discover(&dir, &reserved).await;
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    let host = Arc::new(report.host);

    let provider = ExternalProvider {
        calls: AtomicUsize::new(0),
        requests: Mutex::new(Vec::new()),
    };
    let mut session = SessionLog::create(&dir).expect("session");
    session
        .append(&Event::SessionStart {
            id: session.id().to_string(),
            cwd: dir.display().to_string(),
            model: "external".into(),
            provider: "deepseek".into(),
            ts: now_ms(),
        })
        .expect("start");

    let mut config = AgentConfig::new("external", &dir);
    config.sandbox = SandboxPolicy::new(SandboxMode::FullAccess, NetworkMode::On);
    config.approval = ApprovalMode::Never;
    config.allow_unsandboxed = true;
    config.max_steps = 2;
    config.extension_host = Some(host.clone());

    let outcome = run(
        &provider,
        &mut session,
        &config,
        "use the demo extension",
        &mut |_| true,
        &mut |_| {},
    )
    .await
    .expect("run");
    assert_eq!(outcome.final_text, "done");

    {
        let requests = provider.requests.lock().expect("requests");
        assert!(requests[0]
            .tools
            .iter()
            .any(|tool| tool.function.name == "demo_echo"));
    }

    let loaded = load(session.path()).expect("load");
    assert!(loaded.events.iter().any(|event| matches!(
        event,
        Event::ToolResult { ok: true, output, .. } if output == "echo: hello"
    )));

    host.shutdown().await;
    fs::remove_dir_all(dir).expect("cleanup");
}
