//! Minimal turn loop.
//!
//! The loop owns the conversation order, tool dispatch, and session events. It
//! does not know about wire formats or concrete tools; those live in
//! `niucode-provider` and `niucode-tools`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Result};
use futures_util::StreamExt;
use niucode_core::{now_ms, Event, SessionLog, ToolCall, Usage};
use niucode_provider::{ChatMessage, CompletionRequest, Provider, StreamEvent};
use niucode_tools::{ChangePreview, SandboxPolicy, Toolbox};
use serde_json::{json, Value};

mod context;
mod instructions;
mod subagent;

use context::non_empty;
pub use context::{
    build_handoff, estimate_context, project_history, ContextConfig, Handoff,
    DEFAULT_COMPACT_AT_PERCENT, DEFAULT_CONTEXT_BUDGET, DEFAULT_KEEP_RECENT_TURNS,
};
pub use instructions::{compose_system_prompt, Instructions};

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
    /// Full structured preview for file-changing tools.
    pub preview: Option<ChangePreview>,
}

#[derive(Clone)]
pub struct AgentConfig {
    pub model: String,
    pub max_steps: usize,
    pub system_prompt: String,
    pub working_dir: PathBuf,
    pub sandbox: SandboxPolicy,
    pub approval: ApprovalMode,
    pub allow_unsandboxed: bool,
    pub context: ContextConfig,
    /// When set, only these tool names are advertised and executable.
    pub allowed_tools: Option<Vec<String>>,
    /// Read-only planning mode. The loop still runs, but the prompt tells the
    /// model not to modify files; the TUI also narrows `allowed_tools`.
    pub plan_mode: bool,
    /// Active session goal / completion condition, injected into the system
    /// prompt and preserved across compaction/resume.
    pub goal: Option<String>,
    /// Optional session-scoped sidecar host. External tools are merged into the
    /// advertised tool list and dispatched after the built-ins.
    pub extension_host: Option<Arc<niucode_extension::ExtensionHost>>,
    /// Shared W1 write lock. Parent and Worker subagents use the same lock so
    /// mutating built-ins never race on one workspace.
    pub write_lock: Option<Arc<tokio::sync::Mutex<()>>>,
    /// Optional workspace-relative path prefixes for write/edit. bash is not
    /// path-scoped; it is serialized by `write_lock` instead.
    pub write_scope: Option<Vec<String>>,
}

impl AgentConfig {
    pub fn new(model: impl Into<String>, working_dir: impl Into<PathBuf>) -> Self {
        let working_dir = working_dir.into();
        let system_prompt = format!(
            "You are niucode, a minimal coding agent working inside {}.\n\
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
            context: ContextConfig::default(),
            allowed_tools: None,
            plan_mode: false,
            goal: None,
            extension_host: None,
            write_lock: None,
            write_scope: None,
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
        output: String,
    },
    TurnEnd {
        usage: Usage,
    },
    Compaction {
        replaced_events: usize,
        before_tokens: usize,
        after_tokens: usize,
    },
    Warning(String),
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
    let mut tools = Toolbox::specs();
    if let Some(host) = &config.extension_host {
        tools.extend(host.specs());
    }
    let tools = match &config.allowed_tools {
        Some(allowed) => tools
            .into_iter()
            .filter(|tool| allowed.iter().any(|name| name == &tool.function.name))
            .collect(),
        None => tools,
    };
    let loaded_instructions = instructions::load(&config.working_dir);
    for warning in &loaded_instructions.warnings {
        observer(AgentEvent::Warning(warning.clone()));
    }
    let mut system_prompt = compose_system_prompt(&config.system_prompt, &loaded_instructions.text);
    if let Some(goal) = &config.goal {
        system_prompt.push_str("\n\n## Active goal\n");
        system_prompt.push_str(goal);
        system_prompt.push_str(
            "\n\nKeep working toward this goal. It does not change your permissions; \
             use only the tools currently available.",
        );
    }
    if config.plan_mode {
        system_prompt.push_str(
            "\n\n## Plan mode\nYou are in read-only plan mode. Do not modify files, \
             run mutating commands, or change external state. Investigate the task and \
             return a concrete plan with steps, files, risks, and verification. The user \
             will approve the plan before execution.",
        );
    }

    let mut messages = Vec::with_capacity(history.len() + 2);
    messages.push(ChatMessage::system(system_prompt.clone()));
    messages.extend(history);
    messages.push(ChatMessage::user(prompt));

    if !loaded_instructions.sources.is_empty() {
        session.append(&Event::Instructions {
            sources: loaded_instructions.sources,
            digest: loaded_instructions.digest,
            ts: now_ms(),
        })?;
    }
    session.append(&Event::UserMessage {
        text: prompt.to_string(),
        ts: now_ms(),
    })?;

    let mut total_usage = Usage::default();
    let mut compaction_warned = false;

    for turns in 1..=config.max_steps {
        match maybe_compact(session, config, &system_prompt, &mut messages, &tools)? {
            Some(AgentEvent::Warning(message)) if !compaction_warned => {
                compaction_warned = true;
                observer(AgentEvent::Warning(message));
            }
            Some(AgentEvent::Warning(_)) => {}
            Some(event) => {
                compaction_warned = false;
                observer(event);
            }
            None => {}
        }

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

            let available = config
                .allowed_tools
                .as_ref()
                .is_none_or(|allowed| allowed.iter().any(|name| name == &call.name));
            // W1: one workspace, one write lock. The guard is held through the
            // tool call (including approval) so parent and Worker never race.
            let _write_guard = if matches!(call.name.as_str(), "write" | "edit" | "bash") {
                match &config.write_lock {
                    Some(lock) => Some(lock.lock().await),
                    None => None,
                }
            } else {
                None
            };
            let scope_allowed = write_in_scope(config, &call);
            let result = if !available {
                niucode_tools::ToolResult {
                    ok: false,
                    output: format!("tool {} is not available to this agent", call.name),
                }
            } else if !scope_allowed {
                niucode_tools::ToolResult {
                    ok: false,
                    output: format!(
                        "path is outside this Worker write scope ({}); use a scoped path or ask the parent",
                        call.input.get("path").and_then(Value::as_str).unwrap_or("")
                    ),
                }
            } else if call.name == "subagent" {
                subagent::run_tool(provider, session, config, &call).await
            } else if let Some(host) = config
                .extension_host
                .as_ref()
                .filter(|host| host.has_tool(&call.name))
            {
                // M4-002 external tools are local, read-only sidecars. Risk
                // metadata and per-call approval arrive with the capability
                // profile work.
                host.execute(&call.name, &call.input).await
            } else if requires_approval(&call.name) && config.approval == ApprovalMode::Ask {
                let request = ApprovalRequest {
                    tool: call.name.clone(),
                    summary: approval_summary(&call),
                    preview: matches!(call.name.as_str(), "write" | "edit")
                        .then(|| toolbox.preview(&call.name, &call.input)),
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
                    niucode_tools::ToolResult {
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
                output: result.output.clone(),
            });

            messages.push(ChatMessage::tool(call.id, result.output));
        }
    }

    bail!(
        "max steps reached ({}); the task is not complete",
        config.max_steps
    )
}

fn write_in_scope(config: &AgentConfig, call: &ToolCall) -> bool {
    if !matches!(call.name.as_str(), "write" | "edit") {
        return true;
    }
    let Some(scopes) = &config.write_scope else {
        return true;
    };
    let Some(path) = call.input.get("path").and_then(Value::as_str) else {
        return false;
    };
    let path = path.trim_start_matches("./");
    scopes.iter().any(|scope| {
        let scope = scope.trim().trim_start_matches("./").trim_end_matches('/');
        !scope.is_empty() && (path == scope || path.starts_with(&format!("{scope}/")))
    })
}

fn requires_approval(tool: &str) -> bool {
    matches!(tool, "write" | "edit" | "bash")
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

fn maybe_compact(
    session: &mut SessionLog,
    config: &AgentConfig,
    system_prompt: &str,
    messages: &mut Vec<ChatMessage>,
    tools: &[niucode_provider::ToolSpec],
) -> Result<Option<AgentEvent>> {
    let before = estimate_context(messages, tools);
    if !config.context.should_compact(before) {
        return Ok(None);
    }

    let loaded = match niucode_core::load(session.path()) {
        Ok(loaded) => loaded,
        Err(error) => {
            return Ok(Some(AgentEvent::Warning(format!(
                "compaction skipped: cannot load session: {error:#}"
            ))));
        }
    };
    let handoff = match build_handoff(&loaded.events, config.context.keep_recent_turns) {
        Ok(handoff) => handoff,
        Err(error) => {
            return Ok(Some(AgentEvent::Warning(format!(
                "compaction skipped: {error:#}"
            ))));
        }
    };

    let mut candidate = loaded.events.clone();
    candidate.push(Event::Compaction {
        summary: handoff.summary.clone(),
        upto: handoff.upto as u64,
        before_tokens: before as u64,
        after_tokens: 0,
        ts: now_ms(),
    });
    let history = match project_history(&candidate) {
        Ok(history) => history,
        Err(error) => {
            return Ok(Some(AgentEvent::Warning(format!(
                "compaction skipped: projection failed: {error:#}"
            ))));
        }
    };
    let mut projected = Vec::with_capacity(history.len() + 1);
    projected.push(ChatMessage::system(system_prompt.to_string()));
    projected.extend(history);
    let after = estimate_context(&projected, tools);
    if after >= before {
        return Ok(Some(AgentEvent::Warning(format!(
            "compaction skipped: handoff would not reduce context (~{before} → ~{after} tokens)"
        ))));
    }

    let event = Event::Compaction {
        summary: handoff.summary,
        upto: handoff.upto as u64,
        before_tokens: before as u64,
        after_tokens: after as u64,
        ts: now_ms(),
    };
    if let Err(error) = session.append(&event) {
        return Ok(Some(AgentEvent::Warning(format!(
            "compaction skipped: append failed: {error:#}"
        ))));
    }

    *messages = projected;
    Ok(Some(AgentEvent::Compaction {
        replaced_events: handoff.upto,
        before_tokens: before,
        after_tokens: after,
    }))
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

    use niucode_core::session::load;
    use niucode_provider::{Role, ScriptedProvider, StreamEvent, ToolCallDelta};
    use niucode_tools::{LineKind, NetworkMode, SandboxMode};

    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("niucode-agent-{tag}-{stamp}-{sequence}"));
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
        let preview = preview_seen.expect("preview");
        assert!(preview.is_diff());
        assert_eq!(preview.files()[0].path, "b.txt");
        assert_eq!((preview.added, preview.removed), (1, 0));
        assert!(preview.files()[0].hunks[0]
            .lines
            .iter()
            .any(|line| line.kind == LineKind::Add && line.text == "bad"));
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
        assert_eq!(requests[0].tools.len(), 7);
        assert!(requests[1]
            .messages
            .iter()
            .any(|message| message.role == niucode_provider::Role::Tool));

        let loaded = load(&session_path).expect("load session");
        assert!(loaded
            .events
            .iter()
            .any(|event| matches!(event, Event::ToolResult { ok: true, .. })));

        fs::remove_dir_all(directory).expect("cleanup");
    }
    #[test]
    fn write_scope_requires_a_workspace_prefix() {
        let mut config = AgentConfig::new("test", ".");
        config.write_scope = Some(vec!["src".into(), "crates/app".into()]);
        let call = |path: &str| ToolCall {
            id: "call_1".into(),
            name: "write".into(),
            input: json!({"path": path, "content": "x"}),
        };

        assert!(write_in_scope(&config, &call("src/main.rs")));
        assert!(write_in_scope(&config, &call("./src/lib.rs")));
        assert!(write_in_scope(&config, &call("crates/app/src/main.rs")));
        assert!(!write_in_scope(&config, &call("srcx/main.rs")));
        assert!(!write_in_scope(&config, &call("tests/main.rs")));
        assert!(!write_in_scope(&config, &call("")));
    }
}
