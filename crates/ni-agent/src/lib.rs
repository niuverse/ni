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
use ni_tools::Toolbox;
use serde_json::{json, Value};

pub struct AgentConfig {
    pub model: String,
    pub max_steps: usize,
    pub system_prompt: String,
    pub working_dir: PathBuf,
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
    observer: &mut dyn FnMut(AgentEvent),
) -> Result<RunOutcome> {
    if config.max_steps == 0 {
        bail!("max_steps must be greater than zero");
    }

    let toolbox = Toolbox::new(&config.working_dir)?;
    let tools = Toolbox::specs();
    let mut messages = vec![
        ChatMessage::system(config.system_prompt.clone()),
        ChatMessage::user(prompt),
    ];

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

            let result = toolbox.execute(&call.name, &call.input).await;

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
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use ni_core::session::load;
    use ni_provider::{ScriptedProvider, StreamEvent, ToolCallDelta};

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
        let config = AgentConfig::new("test-model", directory.clone());

        let outcome = run(&provider, &mut session, &config, "do nothing", &mut |_| {})
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
        let mut config = AgentConfig::new("test-model", directory.clone());
        config.max_steps = 1;

        let error = run(
            &provider,
            &mut session,
            &config,
            "read forever",
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
        let config = AgentConfig::new("test-model", directory.clone());
        let mut events = Vec::new();

        let outcome = run(
            &provider,
            &mut session,
            &config,
            "read a.txt",
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
        assert_eq!(requests[0].tools.len(), 4);
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
