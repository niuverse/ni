//! Contract tests for read-only tools supplied by an embedder.

use std::sync::Arc;

use niucode::{AgentEvent, EmbedderTool, ExecOptions, ToolResult, ToolSpec, TurnHooks};
use serde_json::{json, Value};

#[derive(Debug)]
struct EchoTool {
    name: &'static str,
    read_only: bool,
}

#[async_trait::async_trait]
impl EmbedderTool for EchoTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::function(
            self.name,
            "Echo a query back; test-only.",
            json!({
                "type": "object",
                "properties": {"query": {"type": "string"}},
                "required": ["query"]
            }),
        )
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    async fn execute(&self, input: Value) -> ToolResult {
        ToolResult {
            ok: true,
            output: format!(
                "found {}",
                input.get("query").and_then(Value::as_str).unwrap_or("")
            ),
        }
    }
}

#[derive(Default)]
struct ToolRecorder {
    results: Vec<(String, bool, String)>,
}

impl TurnHooks for ToolRecorder {
    fn on_event(&mut self, event: &AgentEvent) {
        if let AgentEvent::ToolResult {
            name, ok, summary, ..
        } = event
        {
            self.results.push((name.clone(), *ok, summary.clone()));
        }
    }
}

fn fixture(directory: &std::path::Path, name: &str) -> std::path::PathBuf {
    let path = directory.join("fixture.json");
    std::fs::write(
        &path,
        json!({
            "schema_version": 1,
            "turns": [
                [{
                    "type": "tool_call_delta",
                    "index": 0,
                    "id": "call-1",
                    "name": name,
                    "arguments": json!({"query": "rust"}).to_string()
                }],
                [{"type": "text_delta", "text": "done"}]
            ]
        })
        .to_string(),
    )
    .unwrap();
    path
}

fn options(
    directory: &std::path::Path,
    fixture: std::path::PathBuf,
    tools: Vec<Arc<dyn EmbedderTool>>,
) -> ExecOptions {
    ExecOptions {
        prompt: "recall".into(),
        cwd: Some(directory.to_path_buf()),
        session_dir: Some(directory.join("sessions")),
        replay: Some(fixture),
        sandbox: "full-access".into(),
        network: "on".into(),
        approval: "never".into(),
        max_steps: 2,
        extra_tools: tools,
        ..ExecOptions::default()
    }
}

#[tokio::test]
async fn an_embedder_tool_is_advertised_dispatched_and_observed() {
    let directory = tempfile::tempdir().unwrap();
    let fixture = fixture(directory.path(), "echo_memory");
    let mut hooks = ToolRecorder::default();
    let code = niucode::run_exec_with_hooks(
        options(
            directory.path(),
            fixture,
            vec![Arc::new(EchoTool {
                name: "echo_memory",
                read_only: true,
            })],
        ),
        &mut hooks,
    )
    .await
    .expect("embedder tool run");
    assert_eq!(code, 0);
    assert_eq!(hooks.results.len(), 1);
    assert_eq!(hooks.results[0].0, "echo_memory");
    assert!(hooks.results[0].1);
    assert!(hooks.results[0].2.contains("found rust"));
}

#[tokio::test]
async fn a_mutating_embedder_tool_is_rejected_before_the_turn() {
    let directory = tempfile::tempdir().unwrap();
    let fixture = fixture(directory.path(), "echo_memory");
    let error = niucode::run_exec_with_hooks(
        options(
            directory.path(),
            fixture,
            vec![Arc::new(EchoTool {
                name: "echo_memory",
                read_only: false,
            })],
        ),
        &mut niucode::NoHooks,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("not read-only"), "{error}");
}

#[tokio::test]
async fn a_name_collision_is_rejected_before_the_turn() {
    let directory = tempfile::tempdir().unwrap();
    let fixture = fixture(directory.path(), "read");
    let error = niucode::run_exec_with_hooks(
        options(
            directory.path(),
            fixture,
            vec![Arc::new(EchoTool {
                name: "read",
                read_only: true,
            })],
        ),
        &mut niucode::NoHooks,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("collides"), "{error}");
}
