//! Minimal turn loop.
//!
//! The loop owns the conversation order, tool dispatch, and session events. It
//! does not know about wire formats or concrete tools; those live in
//! `ni-provider` and `ni-tools`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{bail, Result};
use futures_util::StreamExt;
use ni_core::{now_ms, Event, SessionLog, ToolCall, Usage};
use ni_provider::{ChatMessage, CompletionRequest, Provider, StreamEvent};
use ni_tools::{SandboxPolicy, Toolbox};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalMode {
    Ask,
    OnFailure,
    Never,
}

impl ApprovalMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::OnFailure => "on-failure",
            Self::Never => "never",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub tool: String,
    pub summary: String,
    pub preview: Option<String>,
}

pub struct AgentConfig {
    pub model: String,
    pub max_steps: usize,
    pub system_prompt: String,
    pub working_dir: PathBuf,
    pub sandbox: SandboxPolicy,
    pub approval: ApprovalMode,
    pub allow_unsandboxed: bool,
}

impl AgentConfig {
    pub fn new(model: impl Into<String>, working_dir: impl Into<PathBuf>) -> Self {
        let working_dir = working_dir.into();
        let system_prompt = format!(
            "You are ni, a minimal coding agent working inside {}.\n\
             Use the provided tools to inspect and modify files. Read a file before editing it.\n\
             Prefer small, verifiable changes and run tests with bash when useful.\n\
             When the task is complete, reply with a concise summary of what changed and any remaining issues.\n\
             Do not claim success you have not verified.",
            working_dir.display()
        );

        Self {
            model: model.into(),
            max_steps: 12,
            system_prompt,
            working_dir,
            sandbox: SandboxPolicy::default(),
            approval: ApprovalMode::OnFailure,
            allow_unsandboxed: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AgentEvent {
    TextDelta(String),
    ReasoningDelta(String),
    ToolCall {
        name: String,
        input: Value,
    },
    ToolResult {
        name: String,
        ok: bool,
        summary: String,
    },
    TurnEnd {
        usage: Usage,
    },
}

#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub turns: usize,
    pub usage: Usage,
    pub final_text: String,
}

pub async fn run(
    provider: &dyn Provider,
    session: &mut SessionLog,
    config: &AgentConfig,
    prompt: &str,
    approver: &mut dyn FnMut(&ApprovalRequest) -> bool,
    observer: &mut dyn FnMut(AgentEvent),
) -> Result<RunOutcome> {
    run_with_history(
        provider,
        session,
        config,
        prompt,
        Vec::new(),
        approver,
        observer,
    )
    .await
}

/// Continue with messages reconstructed from an existing session.
pub async fn run_with_history(
    provider: &dyn Provider,
    session: &mut SessionLog,
    config: &AgentConfig,
    prompt: &str,
    history: Vec<ChatMessage>,
    approver: &mut dyn FnMut(&ApprovalRequest) -> bool,
    observer: &mut dyn FnMut(AgentEvent),
) -> Result<RunOutcome> {
    if config.max_steps == 0 {
        bail!("max_steps must be greater than zero");
    }

    let toolbox = Toolbox::with_policy_and_fallback(
        &config.working_dir,
        config.sandbox,
        config.allow_unsandboxed,
    )?;
    let tools = Toolbox::specs();
    let mut messages = Vec::with_capacity(history.len() + 2);
    messages.push(ChatMessage::system(config.system_prompt.clone()));
    messages.extend(history);
    messages.push(ChatMessage::user(prompt));

    session.append(&Event::UserMessage {
        text: prompt.to_string(),
        ts: now_ms(),
    })?;

    let mut total_usage = Usage::default();

    for turns in 1..=config.max_steps {
        let mut request = CompletionRequest::new(config.model.clone());
        request.messages = messages.clone();
        request.tools = tools.clone();

        let mut stream = provider.stream(request).await?;
        let mut text = String::new();
        let mut reasoning = String::new();
        let mut pending = BTreeMap::<u32, PendingCall>::new();
        let mut usage = None;

        while let Some(event) = stream.next().await {
            match event? {
                StreamEvent::TextDelta(delta) => {
                    observer(AgentEvent::TextDelta(delta.clone()));
                    text.push_str(&delta);
                }
                StreamEvent::ReasoningDelta(delta) => {
                    observer(AgentEvent::ReasoningDelta(delta.clone()));
                    reasoning.push_str(&delta);
                }
                StreamEvent::ToolCallDelta(delta) => {
                    let call = pending.entry(delta.index).or_default();
                    if let Some(id) = delta.id {
                        call.id = Some(id);
                    }
                    if let Some(name) = delta.name {
                        call.name = Some(name);
                    }
                    call.arguments.push_str(&delta.arguments);
                }
                StreamEvent::Usage(value) => usage = Some(value),
                StreamEvent::Done { .. } => {}
            }
        }

        let tool_calls = assemble_tool_calls(pending)?;

        session.append(&Event::AssistantMessage {
            text: text.clone(),
            reasoning: non_empty(reasoning.clone()),
            tool_calls: tool_calls.clone(),
            ts: now_ms(),
        })?;

        let turn_usage = usage.unwrap_or_default();
        add_usage(&mut total_usage, &turn_usage);
        session.append(&Event::TurnEnd {
            usage: turn_usage.clone(),
            ts: now_ms(),
        })?;
        observer(AgentEvent::TurnEnd { usage: turn_usage });

        messages.push(ChatMessage::assistant(
            non_empty(text.clone()),
            non_empty(reasoning),
            tool_calls.clone(),
        ));

        if tool_calls.is_empty() {
            session.append(&Event::SessionEnd { ts: now_ms() })?;
            return Ok(RunOutcome {
                turns,
                usage: total_usage,
                final_text: text,
            });
        }

        for call in tool_calls {
            observer(AgentEvent::ToolCall {
                name: call.name.clone(),
                input: call.input.clone(),
            });

            let result = if requires_approval(&call.name) && config.approval == ApprovalMode::Ask {
                let request = ApprovalRequest {
                    tool: call.name.clone(),
                    summary: approval_summary(&call),
                    preview: approval_preview(&call),
                };
                let allowed = approver(&request);
                session.append(&Event::Approval {
                    tool: call.name.clone(),
                    allowed,
                    ts: now_ms(),
                })?;
                if allowed {
                    toolbox.execute(&call.name, &call.input).await
                } else {
                    ni_tools::ToolResult {
                        ok: false,
                        output: "approval denied by user".into(),
                    }
                }
            } else {
                toolbox.execute(&call.name, &call.input).await
            };

            session.append(&Event::ToolResult {
                call_id: call.id.clone(),
                ok: result.ok,
                output: result.output.clone(),
                ts: now_ms(),
            })?;

            observer(AgentEvent::ToolResult {
                name: call.name.clone(),
                ok: result.ok,
                summary: summary(&result.output),
            });

            messages.push(ChatMessage::tool(call.id, result.output));
        }
    }

    bail!(
        "max steps reached ({}); the task is not complete",
        config.max_steps
    )
}

fn requires_approval(tool: &str) -> bool {
    matches!(tool, "write" | "edit" | "bash")
}

fn approval_preview(call: &ToolCall) -> Option<String> {
    let preview = match call.name.as_str() {
        "write" => {
            let content = call.input.get("content").and_then(Value::as_str)?;
            let mut rendered = String::new();
            for line in content.lines().take(40) {
                rendered.push('+');
                rendered.push_str(line);
                rendered.push('\n');
            }
            rendered
        }
        "edit" => {
            let old = call.input.get("old").and_then(Value::as_str)?;
            let new = call.input.get("new").and_then(Value::as_str)?;
            format!("-{old}\n+{new}\n")
        }
        _ => return None,
    };

    let mut truncated: String = preview.chars().take(1_600).collect();
    if preview.chars().count() > 1_600 {
        truncated.push_str("\n... (preview truncated)");
    }
    Some(truncated)
}

fn approval_summary(call: &ToolCall) -> String {
    let detail = match call.name.as_str() {
        "bash" => call.input.get("command").and_then(Value::as_str),
        "write" | "edit" => call.input.get("path").and_then(Value::as_str),
        _ => None,
    };
    let detail = detail.unwrap_or("").chars().take(240).collect::<String>();
    format!("{} {}", call.name, detail).trim().to_string()
}

/// Rebuild provider messages from durable session events.
///
/// `SessionStart`, `TurnEnd`, and `SessionEnd` carry no conversation content.
/// A `Compaction` event becomes a system message so resumed turns retain its
/// handoff text.
pub fn project_history(events: &[Event]) -> Result<Vec<ChatMessage>> {
    let mut messages = Vec::new();
    let mut pending_tools: Vec<String> = Vec::new();

    for event in events {
        match event {
            Event::SessionStart { .. }
            | Event::TurnEnd { .. }
            | Event::Policy { .. }
            | Event::Approval { .. }
            | Event::SessionEnd { .. } => {}
            Event::UserMessage { text, .. } => {
                flush_pending_tools(&mut messages, &mut pending_tools);
                messages.push(ChatMessage::user(text.clone()));
            }
            Event::AssistantMessage {
                text,
                reasoning,
                tool_calls,
                ..
            } => {
                messages.push(ChatMessage::assistant(
                    non_empty(text.clone()),
                    reasoning.clone().filter(|value| !value.is_empty()),
                    tool_calls.clone(),
                ));
                pending_tools = tool_calls.iter().map(|call| call.id.clone()).collect();
            }
            Event::ToolResult {
                call_id, output, ..
            } => {
                pending_tools.retain(|pending| pending != call_id);
                messages.push(ChatMessage::tool(call_id.clone(), output.clone()));
            }
            Event::Compaction { summary, .. } => {
                flush_pending_tools(&mut messages, &mut pending_tools);
                messages.push(ChatMessage::system(format!(
                    "Previous context summary:\n{summary}"
                )));
            }
        }
    }

    flush_pending_tools(&mut messages, &mut pending_tools);
    Ok(messages)
}

fn flush_pending_tools(messages: &mut Vec<ChatMessage>, pending: &mut Vec<String>) {
    for call_id in pending.drain(..) {
        messages.push(ChatMessage::tool(
            call_id,
            "cancelled before the tool produced a result".to_string(),
        ));
    }
}

#[derive(Debug, Default)]
struct PendingCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

fn assemble_tool_calls(pending: BTreeMap<u32, PendingCall>) -> Result<Vec<ToolCall>> {
    pending
        .into_iter()
        .map(|(index, call)| {
            let name = call.name.ok_or_else(|| {
                anyhow::anyhow!("provider emitted tool call {index} without a name")
            })?;
            let id = call.id.unwrap_or_else(|| format!("call_{index}"));
            let input = if call.arguments.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(&call.arguments).unwrap_or(Value::Null)
            };

            Ok(ToolCall { id, name, input })
        })
        .collect()
}

fn add_usage(total: &mut Usage, turn: &Usage) {
    total.input_tokens += turn.input_tokens;
    total.output_tokens += turn.output_tokens;
    total.cached_tokens += turn.cached_tokens;
    total.reasoning_tokens += turn.reasoning_tokens;
}

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn summary(output: &str) -> String {
    output
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(160)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use ni_core::session::load;
    use ni_provider::{Role, ScriptedProvider, StreamEvent, ToolCallDelta};
    use ni_tools::{NetworkMode, SandboxMode};

    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("ni-agent-{tag}-{stamp}-{sequence}"));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create temp dir");
        directory
    }

    #[tokio::test]
    async fn finishes_without_tool_calls() {
        let directory = temp_dir("simple");
        let mut session = SessionLog::create(directory.as_path()).expect("session");
        let session_path = session.path().to_path_buf();
        let provider = ScriptedProvider::from_turns(vec![vec![
            StreamEvent::TextDelta("done".into()),
            StreamEvent::Done {
                finish_reason: Some("stop".into()),
            },
        ]]);
        let config = test_config(&directory);

        let outcome = run(
            &provider,
            &mut session,
            &config,
            "do nothing",
            &mut |_| true,
            &mut |_| {},
        )
        .await
        .expect("run");

        assert_eq!(outcome.final_text, "done");
        assert_eq!(outcome.turns, 1);
        let loaded = load(&session_path).expect("load session");
        assert_eq!(
            loaded.events.iter().map(Event::kind).collect::<Vec<_>>(),
            vec![
                "user_message",
                "assistant_message",
                "turn_end",
                "session_end"
            ]
        );

        fs::remove_dir_all(directory).expect("cleanup");
    }

    fn test_config(directory: &Path) -> AgentConfig {
        let mut config = AgentConfig::new("test-model", directory.to_path_buf());
        config.sandbox = SandboxPolicy::new(SandboxMode::FullAccess, NetworkMode::On);
        config.approval = ApprovalMode::Never;
        config.allow_unsandboxed = true;
        config
    }

    #[tokio::test]
    async fn denies_mutating_tool_without_approval() {
        let directory = temp_dir("approval-deny");
        let mut session = SessionLog::create(directory.as_path()).expect("session");
        let provider = ScriptedProvider::from_turns(vec![
            vec![
                StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 0,
                    id: Some("write_1".into()),
                    name: Some("write".into()),
                    arguments: "{\"path\":\"b.txt\",\"content\":\"bad\"}".into(),
                }),
                StreamEvent::Done {
                    finish_reason: Some("tool_calls".into()),
                },
            ],
            vec![
                StreamEvent::TextDelta("stopped".into()),
                StreamEvent::Done {
                    finish_reason: Some("stop".into()),
                },
            ],
        ]);
        let mut config = test_config(&directory);
        config.approval = ApprovalMode::Ask;
        let mut requested = 0_usize;
        let mut preview_seen = None;
        let mut approver = |request: &ApprovalRequest| {
            requested += 1;
            preview_seen = request.preview.clone();
            false
        };

        let outcome = run(
            &provider,
            &mut session,
            &config,
            "write b.txt",
            &mut approver,
            &mut |_| {},
        )
        .await
        .expect("run");

        assert_eq!(requested, 1);
        assert!(preview_seen
            .as_deref()
            .is_some_and(|preview| preview.contains("+bad")));
        assert_eq!(outcome.final_text, "stopped");
        assert!(!directory.join("b.txt").exists());
        let loaded = load(session.path()).expect("load");
        assert!(loaded
            .events
            .iter()
            .any(|event| matches!(event, Event::Approval { allowed: false, .. })));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn projects_conversation_history_from_events() {
        let events = vec![
            Event::UserMessage {
                text: "first".into(),
                ts: 1,
            },
            Event::AssistantMessage {
                text: "answer".into(),
                reasoning: Some("thought".into()),
                tool_calls: vec![ToolCall {
                    id: "call_1".into(),
                    name: "read".into(),
                    input: json!({"path": "a.txt"}),
                }],
                ts: 2,
            },
            Event::ToolResult {
                call_id: "call_1".into(),
                ok: true,
                output: "file".into(),
                ts: 3,
            },
            Event::TurnEnd {
                usage: Usage::default(),
                ts: 4,
            },
        ];

        let history = project_history(&events).expect("project history");
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].role, Role::User);
        assert_eq!(history[1].role, Role::Assistant);
        assert_eq!(history[1].reasoning_content.as_deref(), Some("thought"));
        assert_eq!(history[2].role, Role::Tool);
        assert_eq!(history[2].tool_call_id.as_deref(), Some("call_1"));
    }

    #[test]
    fn projects_cancelled_tool_call_as_error_result() {
        let events = vec![
            Event::AssistantMessage {
                text: String::new(),
                reasoning: None,
                tool_calls: vec![ToolCall {
                    id: "call_1".into(),
                    name: "bash".into(),
                    input: json!({"command": "sleep 30"}),
                }],
                ts: 1,
            },
            Event::SessionEnd { ts: 2 },
            Event::UserMessage {
                text: "next".into(),
                ts: 3,
            },
        ];

        let history = project_history(&events).expect("project history");
        assert_eq!(history.len(), 3);
        assert_eq!(history[1].role, Role::Tool);
        assert!(history[1]
            .content
            .as_deref()
            .is_some_and(|content| content.contains("cancelled")));
        assert_eq!(history[2].role, Role::User);
    }

    #[tokio::test]
    async fn resumes_from_projected_history() {
        let directory = temp_dir("resume");
        let mut session = SessionLog::create(directory.as_path()).expect("session");
        let first = ScriptedProvider::from_turns(vec![vec![
            StreamEvent::TextDelta("first".into()),
            StreamEvent::Done {
                finish_reason: Some("stop".into()),
            },
        ]]);
        let config = test_config(&directory);
        run(
            &first,
            &mut session,
            &config,
            "first task",
            &mut |_| true,
            &mut |_| {},
        )
        .await
        .expect("first run");

        let loaded = load(session.path()).expect("load session");
        let history = project_history(&loaded.events).expect("project");
        let second = ScriptedProvider::from_turns(vec![vec![
            StreamEvent::TextDelta("second".into()),
            StreamEvent::Done {
                finish_reason: Some("stop".into()),
            },
        ]]);
        let outcome = run_with_history(
            &second,
            &mut session,
            &config,
            "second task",
            history,
            &mut |_| true,
            &mut |_| {},
        )
        .await
        .expect("resume run");

        assert_eq!(outcome.final_text, "second");
        let requests = second.requests();
        assert!(requests[0]
            .messages
            .iter()
            .any(|message| message.content.as_deref() == Some("first task")));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[tokio::test]
    async fn fixes_a_failing_fixture_with_scripted_tools() {
        let directory = temp_dir("fixture-fix");
        fs::write(directory.join("test.sh"), "#!/usr/bin/env bash\nexit 1\n").expect("fixture");
        let mut session = SessionLog::create(directory.as_path()).expect("session");
        let provider = ScriptedProvider::from_turns(vec![
            vec![
                StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 0,
                    id: Some("read_1".into()),
                    name: Some("read".into()),
                    arguments: "{\"path\":\"test.sh\"}".into(),
                }),
                StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 1,
                    id: Some("edit_1".into()),
                    name: Some("edit".into()),
                    arguments: "{\"path\":\"test.sh\",\"old\":\"exit 1\",\"new\":\"exit 0\"}"
                        .into(),
                }),
                StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 2,
                    id: Some("bash_1".into()),
                    name: Some("bash".into()),
                    arguments: "{\"command\":\"bash test.sh\"}".into(),
                }),
                StreamEvent::Done {
                    finish_reason: Some("tool_calls".into()),
                },
            ],
            vec![
                StreamEvent::TextDelta("fixed".into()),
                StreamEvent::Done {
                    finish_reason: Some("stop".into()),
                },
            ],
        ]);
        let config = test_config(&directory);

        let outcome = run(
            &provider,
            &mut session,
            &config,
            "fix the failing test",
            &mut |_| true,
            &mut |_| {},
        )
        .await
        .expect("run");

        assert_eq!(outcome.final_text, "fixed");
        assert!(outcome.turns >= 2);
        let test_script = fs::read_to_string(directory.join("test.sh")).expect("read fixed script");
        assert!(test_script.contains("exit 0"));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[tokio::test]
    async fn enforces_max_steps() {
        let directory = temp_dir("max-steps");
        let mut session = SessionLog::create(directory.as_path()).expect("session");
        let provider = ScriptedProvider::from_turns(vec![vec![
            StreamEvent::ToolCallDelta(ToolCallDelta {
                index: 0,
                id: Some("call_1".into()),
                name: Some("read".into()),
                arguments: "{\"path\":\"a.txt\"}".into(),
            }),
            StreamEvent::Done {
                finish_reason: Some("tool_calls".into()),
            },
        ]]);
        fs::write(directory.join("a.txt"), "hello").expect("fixture");
        let mut config = test_config(&directory);
        config.max_steps = 1;

        let error = run(
            &provider,
            &mut session,
            &config,
            "read forever",
            &mut |_| true,
            &mut |_| {},
        )
        .await
        .expect_err("max steps must fail");

        assert!(error.to_string().contains("max steps"));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[tokio::test]
    async fn executes_a_tool_and_continues() {
        let directory = temp_dir("tool");
        fs::write(directory.join("a.txt"), "hello\n").expect("fixture");
        let mut session = SessionLog::create(directory.as_path()).expect("session");
        let session_path = session.path().to_path_buf();
        let provider = ScriptedProvider::from_turns(vec![
            vec![
                StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 0,
                    id: Some("call_1".into()),
                    name: Some("read".into()),
                    arguments: "{\"path\":\"a.txt\"}".into(),
                }),
                StreamEvent::Done {
                    finish_reason: Some("tool_calls".into()),
                },
            ],
            vec![
                StreamEvent::TextDelta("read it".into()),
                StreamEvent::Done {
                    finish_reason: Some("stop".into()),
                },
            ],
        ]);
        let config = test_config(&directory);
        let mut events = Vec::new();

        let outcome = run(
            &provider,
            &mut session,
            &config,
            "read a.txt",
            &mut |_| true,
            &mut |event| events.push(event),
        )
        .await
        .expect("run");

        assert_eq!(outcome.final_text, "read it");
        assert_eq!(outcome.turns, 2);
        assert!(events
            .iter()
            .any(|event| matches!(event, AgentEvent::ToolResult { ok: true, .. })));

        let requests = provider.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].tools.len(), 6);
        assert!(requests[1]
            .messages
            .iter()
            .any(|message| message.role == ni_provider::Role::Tool));

        let loaded = load(&session_path).expect("load session");
        assert!(loaded
            .events
            .iter()
            .any(|event| matches!(event, Event::ToolResult { ok: true, .. })));

        fs::remove_dir_all(directory).expect("cleanup");
    }
}
