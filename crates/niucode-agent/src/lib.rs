//! Minimal turn loop.
//!
//! The loop owns the conversation order, tool dispatch, and session events. It
//! does not know about wire formats or concrete tools; those live in
//! `niucode-provider` and `niucode-tools`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use async_trait::async_trait;
use base64::Engine;
use futures_util::StreamExt;
use niucode_attachments::{check_budget, AttachmentStore};
use niucode_computer::{
    parse_action, ComputerAction, ComputerBackend, ComputerPolicy, ComputerSession, PolicyDecision,
    WindowId, WindowKind,
};
use niucode_core::{now_ms, AttachmentRef, Event, SessionLog, ToolCall, Usage};
use niucode_provider::{
    ChatMessage, CompletionRequest, ContentPart, MessageContent, Provider, ProviderError,
    StreamEvent, ToolSpec,
};
use niucode_tools::{ChangePreview, SandboxPolicy, ToolResult, Toolbox};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

mod changes;
mod context;
mod instructions;
mod runtime;
mod subagent;
pub use changes::{task_review, task_review_by_id, CommandCheck, ReviewedFile, TaskReview};
pub use runtime::{
    pending_inputs, run_interactive, run_interactive_with_attachments, runtime_channel, Cancelled,
    PendingInput, RuntimeClient, RuntimeCommand, RuntimeEndpoint, RuntimeEvent,
};

const MAX_PROVIDER_RETRIES: usize = 3;
const MAX_TOOL_OUTPUT_BYTES: usize = 1_048_576;
const TOOL_OUTPUT_HEAD_BYTES: usize = 65_536;
const TOOL_OUTPUT_TAIL_BYTES: usize = 65_536;
const RETRY_BASE_MS: u64 = 250;
const MAX_RETRY_DELAY_SECS: u64 = 10;
const UNBOUNDED_SAFETY_TURNS: usize = 256;
const MCP_CALL_TIMEOUT: Duration = Duration::from_secs(30);

use context::non_empty;
pub use context::{
    build_handoff, estimate_context, project_history, project_history_with_attachments,
    ContextConfig, Handoff, DEFAULT_COMPACT_AT_PERCENT, DEFAULT_CONTEXT_BUDGET,
    DEFAULT_KEEP_RECENT_TURNS,
};
pub use instructions::{compose_system_prompt, load as load_instructions, Instructions};

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
    /// Optional target screenshot (data URL) for computer approvals. Never
    /// persisted; bounded before it enters the runtime event.
    pub image: Option<String>,
}

/// A read-only tool supplied by an embedder.
///
/// The loop advertises the spec and dispatches by name. v1 accepts only
/// read-only tools: they do not ask for approval, do not write through the
/// sandbox, and are not inherited by subagents.
pub struct ComputerRuntime {
    backend: Box<dyn ComputerBackend>,
    session: ComputerSession,
    last_frame: Option<(String, String)>,
}

impl ComputerRuntime {
    pub fn new(backend: Box<dyn ComputerBackend>, policy: ComputerPolicy) -> Self {
        Self {
            backend,
            session: ComputerSession::new(policy),
            last_frame: None,
        }
    }

    pub fn policy(&self) -> ComputerPolicy {
        self.session.policy()
    }

    pub fn set_policy(&mut self, policy: ComputerPolicy) {
        self.session.set_policy(policy);
    }

    pub fn arm(&mut self) -> anyhow::Result<()> {
        self.session.arm().map_err(Into::into)
    }

    pub fn is_armed(&self) -> bool {
        self.session.is_armed()
    }

    pub fn disarm(&mut self) {
        self.session.disarm();
    }

    /// Emergency stop: turn policy off, disarm, release held input. Returns the
    /// backend release error (if any) after the session is already stopped.
    pub fn emergency_stop(&mut self) -> anyhow::Result<()> {
        self.session.set_policy(ComputerPolicy::Off);
        self.session.disarm();
        self.backend
            .emergency_stop()
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    pub fn observe(&mut self, window: Option<WindowId>) -> Result<niucode_computer::Capture> {
        let capture = self
            .backend
            .observe(window)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        self.session.remember(capture.observation.clone());
        self.last_frame = approval_image(&capture.png)
            .map(|url| (capture.observation.id.as_str().to_string(), url));
        Ok(capture)
    }

    pub fn decide(&self, action: &ComputerAction) -> Result<PolicyDecision> {
        self.session.decide(action).map_err(Into::into)
    }

    pub fn validate(
        &self,
        action: &ComputerAction,
        now_ms: u64,
    ) -> Result<niucode_computer::Observation> {
        Ok(self.session.validate(action, now_ms)?.clone())
    }

    pub fn perform(
        &mut self,
        action: &ComputerAction,
        observation: &niucode_computer::Observation,
    ) -> Result<()> {
        self.backend
            .perform(action, observation)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(())
    }

    pub fn frame_for(&self, action: &ComputerAction) -> Option<String> {
        let id = action.observation_id()?;
        self.last_frame
            .as_ref()
            .filter(|(stored, _)| stored == id.as_str())
            .map(|(_, url)| url.clone())
    }

    pub fn window_for(&self, action: &ComputerAction) -> Option<niucode_computer::WindowInfo> {
        let id = action.observation_id()?;
        self.session
            .observation(id)
            .ok()
            .map(|observation| observation.window.clone())
    }
}

#[async_trait]
pub trait EmbedderTool: Send + Sync + std::fmt::Debug {
    fn spec(&self) -> ToolSpec;
    fn read_only(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value) -> ToolResult;
}

#[derive(Clone)]
pub struct AgentConfig {
    pub provider: String,
    pub model: String,
    pub max_steps: usize,
    pub system_prompt: String,
    pub working_dir: PathBuf,
    pub sandbox: SandboxPolicy,
    pub approval: ApprovalMode,
    pub allow_unsandboxed: bool,
    pub context: ContextConfig,
    /// Recompute the context budget from the selected model catalog on each turn.
    pub context_budget_auto: bool,
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
    /// Read-only tools supplied by an embedder. Dispatched after built-ins and
    /// the extension host; not inherited by subagents.
    pub extra_tools: Vec<Arc<dyn EmbedderTool>>,
    /// Native desktop controller. Advertised as the `computer` tool when set.
    pub computer: Option<Arc<std::sync::Mutex<ComputerRuntime>>>,
    /// Persist full typed computer text in the JSONL. Off by default; the
    /// assistant tool call is otherwise reduced to length + SHA-256.
    pub computer_log_input: bool,
    /// Persist observation PNGs in the session attachment store. Off by
    /// default; screenshots can contain secrets.
    pub computer_keep_frames: bool,
    /// Mounted read-only MCP servers. Dispatched after built-ins and the
    /// extension host; not inherited by subagents.
    pub mcp_host: Option<Arc<niucode_mcp::McpHost>>,
    /// Shared W1 write lock. Parent and Worker subagents use the same lock so
    /// mutating built-ins never race on one workspace.
    pub write_lock: Option<Arc<tokio::sync::Mutex<()>>>,
    /// Optional workspace-relative path prefixes for write/edit. bash is not
    /// path-scoped; it is serialized by `write_lock` instead.
    pub write_scope: Option<Vec<String>>,
    /// Extra read-only roots (trusted skill bodies).
    pub read_roots: Vec<PathBuf>,
    /// Global workspace worktree root for isolated Worker subagents.
    pub worktree_root: Option<PathBuf>,
    /// Explicit opt-in for subagent `profile = "inherit"`. Off by default;
    /// inherit still strips the nested `subagent` tool.
    pub allow_subagent_inheritance: bool,
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
            provider: "deepseek".into(),
            model: model.into(),
            // 0 means no user-facing hard cap. The loop still has a high
            // safety ceiling; normal termination is the model's final answer.
            max_steps: 0,
            system_prompt,
            working_dir,
            sandbox: SandboxPolicy::default(),
            approval: ApprovalMode::OnFailure,
            allow_unsandboxed: false,
            context: ContextConfig::default(),
            context_budget_auto: false,
            allowed_tools: None,
            plan_mode: false,
            goal: None,
            extension_host: None,
            extra_tools: Vec::new(),
            computer: None,
            computer_log_input: false,
            computer_keep_frames: false,
            mcp_host: None,
            write_lock: Some(Arc::new(tokio::sync::Mutex::new(()))),
            write_scope: None,
            read_roots: Vec::new(),
            worktree_root: None,
            allow_subagent_inheritance: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopPhase {
    Project,
    Request,
    Stream,
    Tool,
    Turn,
    Done,
}

impl LoopPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Request => "request",
            Self::Stream => "stream",
            Self::Tool => "tool",
            Self::Turn => "turn",
            Self::Done => "done",
        }
    }
}

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Phase(LoopPhase),
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
    run_inner(
        provider,
        session,
        config,
        prompt,
        history,
        Vec::new(),
        None,
        approver,
        observer,
    )
    .await
}

/// Run one turn with pre-ingested attachments. The store resolves image bytes
/// to OpenAI-compatible data URLs and text attachments to fenced context.
#[allow(clippy::too_many_arguments)]
pub async fn run_with_attachments(
    provider: &dyn Provider,
    session: &mut SessionLog,
    config: &AgentConfig,
    prompt: &str,
    attachments: Vec<AttachmentRef>,
    store: &AttachmentStore,
    approver: &mut dyn FnMut(&ApprovalRequest) -> bool,
    observer: &mut dyn FnMut(AgentEvent),
) -> Result<RunOutcome> {
    let history = crate::context::project_history_with_attachments(
        &niucode_core::load(session.path())?.events,
        store,
        2,
    )?;
    run_inner(
        provider,
        session,
        config,
        prompt,
        history,
        attachments,
        Some(store),
        approver,
        observer,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn run_inner(
    provider: &dyn Provider,
    session: &mut SessionLog,
    config: &AgentConfig,
    prompt: &str,
    history: Vec<ChatMessage>,
    attachments: Vec<AttachmentRef>,
    store: Option<&AttachmentStore>,
    approver: &mut dyn FnMut(&ApprovalRequest) -> bool,
    observer: &mut dyn FnMut(AgentEvent),
) -> Result<RunOutcome> {
    if !attachments.is_empty() {
        check_budget(&attachments, config.context.budget_tokens)?;
    }
    let mut io = runtime::RunIo::callbacks(approver, observer);
    runtime::execute_task(
        provider,
        session,
        config,
        prompt,
        history,
        attachments,
        store,
        &mut io,
    )
    .await
}

fn attachment_message(
    store: Option<&AttachmentStore>,
    prompt: &str,
    attachments: &[AttachmentRef],
    context_budget_tokens: usize,
) -> Result<ChatMessage> {
    if attachments.is_empty() {
        return Ok(ChatMessage::user(prompt));
    }
    check_budget(attachments, context_budget_tokens)?;
    let store = store.ok_or_else(|| anyhow::anyhow!("attachments require an attachment store"))?;
    let mut text = prompt.to_string();
    let mut images = Vec::new();
    for attachment in attachments {
        match attachment.kind {
            niucode_core::AttachmentKind::Image => {
                images.push(store.image_data_url(attachment)?);
            }
            niucode_core::AttachmentKind::Text => {
                let content = store.text(attachment)?;
                text.push_str(&format!(
                    "\n\n<attachment name={:?}>\n````\n{}\n````\n</attachment>",
                    attachment.name.as_deref().unwrap_or("attachment"),
                    content.replace("````", "` ` ` `")
                ));
            }
        }
    }
    if images.is_empty() {
        Ok(ChatMessage::user(text))
    } else {
        Ok(ChatMessage::user_with_images(text, images))
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_engine(
    provider: &dyn Provider,
    session: &SessionLog,
    config: &AgentConfig,
    prompt: &str,
    history: Vec<ChatMessage>,
    attachments: &[AttachmentRef],
    store: Option<&AttachmentStore>,
    io: &mut runtime::RunIo<'_>,
) -> Result<RunOutcome> {
    let toolbox = Toolbox::with_policy_and_roots(
        &config.working_dir,
        config.sandbox,
        config.allow_unsandboxed,
        config.read_roots.clone(),
    )?;
    let mut tools = Toolbox::specs();
    if let Some(host) = &config.extension_host {
        tools.extend(host.specs());
    }
    if let Some(host) = &config.mcp_host {
        for spec in host.specs() {
            if tools
                .iter()
                .any(|existing| existing.function.name == spec.function.name)
            {
                bail!(
                    "MCP tool {} collides with an existing tool",
                    spec.function.name
                );
            }
            tools.push(spec);
        }
    }
    for tool in &config.extra_tools {
        let spec = tool.spec();
        if !tool.read_only() {
            bail!(
                "embedder tool {} is not read-only; v1 supports read-only tools only",
                spec.function.name
            );
        }
        if tools
            .iter()
            .any(|existing| existing.function.name == spec.function.name)
        {
            bail!(
                "embedder tool {} collides with an existing tool",
                spec.function.name
            );
        }
        tools.push(spec);
    }
    if let Some(runtime) = &config.computer {
        let policy = runtime.lock().unwrap().policy();
        let description = if policy == ComputerPolicy::Off {
            "Native desktop control is present but the session policy is off; ask the user to arm it."
        } else {
            "Observe the native desktop and perform observation-bound click/type/key/scroll actions. Every input action is permission-checked and may require approval."
        };
        tools.push(ToolSpec::function(
            "computer",
            description,
            niucode_computer::tool_spec(),
        ));
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
        io.observe(AgentEvent::Warning(warning.clone())).await?;
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

    io.observe(AgentEvent::Phase(LoopPhase::Project)).await?;
    let mut messages = Vec::with_capacity(history.len() + 2);
    messages.push(ChatMessage::system(system_prompt.clone()));
    messages.extend(history);
    messages.push(attachment_message(
        store,
        prompt,
        attachments,
        config.context.budget_tokens,
    )?);

    if !loaded_instructions.sources.is_empty() {
        session.append(&Event::Instructions {
            sources: loaded_instructions.sources,
            digest: loaded_instructions.digest,
            ts: now_ms(),
        })?;
    }

    let mut total_usage = Usage::default();
    let mut compaction_warned = false;

    let turn_limit = if config.max_steps == 0 {
        UNBOUNDED_SAFETY_TURNS
    } else {
        config.max_steps
    };
    for turns in 1..=turn_limit {
        io.inject(session, &mut messages).await?;
        io.observe(AgentEvent::Phase(LoopPhase::Request)).await?;
        match maybe_compact(
            session,
            config,
            &system_prompt,
            &mut messages,
            &tools,
            false,
        )? {
            Some(AgentEvent::Warning(message)) if !compaction_warned => {
                compaction_warned = true;
                io.observe(AgentEvent::Warning(message)).await?;
            }
            Some(AgentEvent::Warning(_)) => {}
            Some(event) => {
                compaction_warned = false;
                io.observe(event).await?;
            }
            None => {}
        }

        cap_visual_budget(&mut messages, MAX_VISUAL_PARTS);
        let mut request = CompletionRequest::new(config.model.clone());
        request.messages = messages.clone();
        request.tools = tools.clone();

        let mut attempt = 0_usize;
        let (text, reasoning, pending, usage, finish_reason) = loop {
            let mut stream = match io.wait(session, provider.stream(request.clone())).await? {
                Ok(stream) => stream,
                Err(error) => {
                    if error.is_context_overflow() && attempt == 0 {
                        if let Some(AgentEvent::Compaction {
                            replaced_events,
                            before_tokens,
                            after_tokens,
                        }) = maybe_compact(
                            session,
                            config,
                            &system_prompt,
                            &mut messages,
                            &tools,
                            true,
                        )? {
                            io.observe(AgentEvent::Compaction {
                                replaced_events,
                                before_tokens,
                                after_tokens,
                            })
                            .await?;
                            request.messages = messages.clone();
                            attempt += 1;
                            continue;
                        }
                    }
                    if error.is_retryable() && attempt < MAX_PROVIDER_RETRIES {
                        attempt += 1;
                        io.wait(session, tokio::time::sleep(retry_delay(&error, attempt)))
                            .await?;
                        continue;
                    }
                    return Err(error.into());
                }
            };

            let mut text = String::new();
            let mut reasoning = String::new();
            let mut pending = BTreeMap::<u32, PendingCall>::new();
            let mut usage = None;
            let mut emitted = false;
            let mut stream_error = None;
            let mut finish_reason = None;

            while let Some(event) = io.wait(session, stream.next()).await? {
                match event {
                    Ok(StreamEvent::TextDelta(delta)) => {
                        emitted = true;
                        io.observe(AgentEvent::TextDelta(delta.clone())).await?;
                        text.push_str(&delta);
                    }
                    Ok(StreamEvent::ReasoningDelta(delta)) => {
                        emitted = true;
                        io.observe(AgentEvent::ReasoningDelta(delta.clone()))
                            .await?;
                        reasoning.push_str(&delta);
                    }
                    Ok(StreamEvent::ToolCallDelta(delta)) => {
                        emitted = true;
                        let call = pending.entry(delta.index).or_default();
                        if let Some(id) = delta.id {
                            call.id = Some(id);
                        }
                        if let Some(name) = delta.name {
                            call.name = Some(name);
                        }
                        call.arguments.push_str(&delta.arguments);
                    }
                    Ok(StreamEvent::Usage(value)) => {
                        emitted = true;
                        usage = Some(value);
                    }
                    Ok(StreamEvent::Done {
                        finish_reason: reason,
                    }) => {
                        emitted = true;
                        finish_reason = reason;
                    }
                    Err(error) => {
                        stream_error = Some(error);
                        break;
                    }
                }
            }

            if let Some(error) = stream_error {
                if error.is_context_overflow() && attempt == 0 && !emitted {
                    if let Some(AgentEvent::Compaction {
                        replaced_events,
                        before_tokens,
                        after_tokens,
                    }) =
                        maybe_compact(session, config, &system_prompt, &mut messages, &tools, true)?
                    {
                        io.observe(AgentEvent::Compaction {
                            replaced_events,
                            before_tokens,
                            after_tokens,
                        })
                        .await?;
                        request.messages = messages.clone();
                        attempt += 1;
                        continue;
                    }
                }
                if !emitted && error.is_retryable() && attempt < MAX_PROVIDER_RETRIES {
                    attempt += 1;
                    io.wait(session, tokio::time::sleep(retry_delay(&error, attempt)))
                        .await?;
                    continue;
                }
                return Err(error.into());
            }

            break (text, reasoning, pending, usage, finish_reason);
        };

        // A length-limited response is incomplete even when its tool arguments
        // happen to parse. Preserve the response and usage, but dispatch nothing.
        let truncated = finish_reason.as_deref() == Some("length");
        let tool_calls = if truncated {
            Vec::new()
        } else {
            assemble_tool_calls(pending)?
        };

        let mut logged_calls = tool_calls.clone();
        redact_tool_calls_for_log(&mut logged_calls, config.computer_log_input);
        session.append(&Event::AssistantMessage {
            text: text.clone(),
            reasoning: non_empty(reasoning.clone()),
            tool_calls: logged_calls,
            ts: now_ms(),
        })?;

        let turn_usage = usage.unwrap_or_default();
        add_usage(&mut total_usage, &turn_usage);
        session.append(&Event::TurnEnd {
            usage: turn_usage.clone(),
            finish_reason,
            ts: now_ms(),
        })?;
        io.observe(AgentEvent::TurnEnd { usage: turn_usage })
            .await?;

        if truncated {
            bail!("model output truncated (finish_reason=length); task incomplete; review recorded output before continuing");
        }

        messages.push(ChatMessage::assistant(
            non_empty(text.clone()),
            non_empty(reasoning),
            tool_calls.clone(),
        ));

        if tool_calls.is_empty() && !io.inject(session, &mut messages).await? {
            session.append(&Event::SessionEnd { ts: now_ms() })?;
            return Ok(RunOutcome {
                turns,
                usage: total_usage,
                final_text: text,
            });
        }

        for call in tool_calls {
            io.observe(AgentEvent::ToolCall {
                name: call.name.clone(),
                input: call.input.clone(),
            })
            .await?;

            let available = config
                .allowed_tools
                .as_ref()
                .is_none_or(|allowed| allowed.iter().any(|name| name == &call.name));
            io.observe(AgentEvent::Phase(LoopPhase::Tool)).await?;
            let scope_allowed = write_in_scope(config, &call);
            let mut tool_images = Vec::new();
            let mut result = if !available {
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
                io.wait(
                    session,
                    subagent::run_tool(provider, session, config, &call),
                )
                .await?
            } else if call.name == "computer" {
                let execution =
                    execute_computer(provider, session, config, io, &call, &mut messages).await?;
                tool_images = execution.images;
                execution.result
            } else if let Some(host) = config
                .extension_host
                .as_ref()
                .filter(|host| host.has_tool(&call.name))
            {
                let _guard = if !host.read_only_tools().contains(&call.name) {
                    match &config.write_lock {
                        Some(lock) => Some(io.wait(session, lock.lock()).await?),
                        None => None,
                    }
                } else {
                    None
                };
                io.wait(session, host.execute(&call.name, &call.input))
                    .await?
            } else if let Some(host) = config
                .mcp_host
                .as_ref()
                .filter(|host| host.has_tool(&call.name))
            {
                let mutating = host.is_mutating(&call.name);
                if mutating && config.approval == ApprovalMode::Never {
                    ToolResult {
                        ok: false,
                        output:
                            "mutating MCP requires an interactive approval mode; use --approval ask"
                                .into(),
                    }
                } else {
                    if mutating {
                        let server = host.server_for(&call.name).unwrap_or("unknown");
                        let preview = bounded_json(&call.input);
                        let request = ApprovalRequest {
                            tool: call.name.clone(),
                            summary: format!(
                                "mutating MCP call on {server}: {}. Effect is server-defined. Arguments: {preview}",
                                call.name
                            ),
                            preview: None,
                            image: None,
                        };
                        let allowed = io.approve(session, &call.id, request).await?;
                        session.append(&Event::Approval {
                            tool: call.name.clone(),
                            allowed,
                            ts: now_ms(),
                        })?;
                        if !allowed {
                            ToolResult {
                                ok: false,
                                output: "approval denied by user".into(),
                            }
                        } else {
                            let _guard = match &config.write_lock {
                                Some(lock) => Some(io.wait(session, lock.lock()).await?),
                                None => None,
                            };
                            mcp_call_result(
                                io.wait(
                                    session,
                                    host.call(&call.name, call.input.clone(), MCP_CALL_TIMEOUT),
                                )
                                .await,
                            )
                        }
                    } else {
                        mcp_call_result(
                            io.wait(
                                session,
                                host.call(&call.name, call.input.clone(), MCP_CALL_TIMEOUT),
                            )
                            .await,
                        )
                    }
                }
            } else if let Some(tool) = config
                .extra_tools
                .iter()
                .find(|tool| tool.spec().function.name == call.name)
            {
                io.wait(session, tool.execute(call.input.clone())).await?
            } else {
                execute_builtin(session, config, io, &toolbox, &call)
                    .await?
                    .result
            };

            result.output = cap_tool_output(result.output);
            session.append(&Event::ToolResult {
                call_id: call.id.clone(),
                ok: result.ok,
                output: result.output.clone(),
                ts: now_ms(),
            })?;

            io.observe(AgentEvent::ToolResult {
                name: call.name.clone(),
                ok: result.ok,
                summary: summary(&result.output),
                output: result.output.clone(),
            })
            .await?;

            if tool_images.is_empty() {
                messages.push(ChatMessage::tool(call.id, result.output));
            } else {
                messages.push(ChatMessage::tool_with_images(
                    call.id,
                    result.output,
                    tool_images,
                ));
            }
        }
    }

    if config.max_steps == 0 {
        bail!("safety turn cap reached ({UNBOUNDED_SAFETY_TURNS}); the task is not complete")
    } else {
        bail!(
            "max steps reached ({}); the task is not complete",
            config.max_steps
        )
    }
}

struct ComputerExecution {
    result: niucode_tools::ToolResult,
    images: Vec<String>,
}

fn audit_computer(
    session: &SessionLog,
    action: &ComputerAction,
    purpose: &str,
    result: &niucode_tools::ToolResult,
) -> Result<()> {
    audit_computer_target(
        session,
        action,
        purpose,
        result,
        action.window_id(),
        action.observation_id().map(ToString::to_string),
    )
}

fn audit_computer_target(
    session: &SessionLog,
    action: &ComputerAction,
    purpose: &str,
    result: &niucode_tools::ToolResult,
    window_id: Option<WindowId>,
    observation_id: Option<String>,
) -> Result<()> {
    let (text_chars, text_sha256) = match action {
        ComputerAction::Type { text, .. } => {
            let digest: String = Sha256::digest(text.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            (text.chars().count() as u64, Some(digest))
        }
        _ => (0, None),
    };
    session.append(&Event::ComputerAction {
        action: action_label(action).to_string(),
        window_id: window_id.map(|window| window.0).unwrap_or(0),
        observation_id: observation_id.unwrap_or_default(),
        purpose: clean_display(purpose, 200),
        ok: result.ok,
        error: (!result.ok).then(|| clean_display(&result.output, 240)),
        text_chars,
        text_sha256,
        ts: now_ms(),
    })?;
    Ok(())
}

async fn execute_computer(
    _provider: &dyn Provider,
    session: &SessionLog,
    config: &AgentConfig,
    io: &mut runtime::RunIo<'_>,
    call: &ToolCall,
    messages: &mut [ChatMessage],
) -> Result<ComputerExecution> {
    let action = match parse_action(&call.input) {
        Ok(action) => action,
        Err(error) => {
            return Ok(ComputerExecution {
                result: niucode_tools::ToolResult {
                    ok: false,
                    output: format!("computer input rejected: {error}"),
                },
                images: Vec::new(),
            });
        }
    };
    let purpose = call
        .input
        .get("purpose")
        .and_then(Value::as_str)
        .unwrap_or("no purpose provided")
        .to_string();
    let execution = execute_computer_action(
        _provider, session, config, io, call, messages, &action, &purpose,
    )
    .await?;
    if !(matches!(action, ComputerAction::Observe { .. }) && execution.result.ok) {
        audit_computer(session, &action, &purpose, &execution.result)?;
    }
    Ok(execution)
}

#[allow(clippy::too_many_arguments)]
async fn execute_computer_action(
    _provider: &dyn Provider,
    session: &SessionLog,
    config: &AgentConfig,
    io: &mut runtime::RunIo<'_>,
    call: &ToolCall,
    messages: &mut [ChatMessage],
    action: &ComputerAction,
    purpose: &str,
) -> Result<ComputerExecution> {
    let Some(runtime) = &config.computer else {
        return Ok(ComputerExecution {
            result: niucode_tools::ToolResult {
                ok: false,
                output: "computer tool is not configured".into(),
            },
            images: Vec::new(),
        });
    };
    let decision = match runtime.lock().unwrap().decide(action) {
        Ok(decision) => decision,
        Err(error) => {
            return Ok(ComputerExecution {
                result: niucode_tools::ToolResult {
                    ok: false,
                    output: format!("computer action rejected: {error}"),
                },
                images: Vec::new(),
            });
        }
    };
    match decision {
        PolicyDecision::Denied => {
            return Ok(ComputerExecution {
                result: niucode_tools::ToolResult {
                    ok: false,
                    output: "computer permission denied; ask the user to arm computer use and grant the required policy".into(),
                },
                images: Vec::new(),
            });
        }
        PolicyDecision::NeedsApproval => {
            let (window, approval_frame) = {
                let runtime = runtime.lock().unwrap();
                (
                    runtime.window_for(action).map(|window| {
                        format!(
                            "{} — {}",
                            clean_display(&window.app, 60),
                            clean_display(&window.title, 80)
                        )
                    }),
                    runtime.frame_for(action),
                )
            };
            let window = window.unwrap_or_else(|| "unobserved window".into());
            let summary = format!(
                "computer {} on {window}. Purpose: {}",
                action_detail(action),
                clean_display(purpose, 200)
            );
            if !io
                .approve(
                    session,
                    &call.id,
                    ApprovalRequest {
                        tool: "computer".into(),
                        summary,
                        preview: None,
                        image: approval_frame,
                    },
                )
                .await?
            {
                session.append(&Event::Approval {
                    tool: "computer".into(),
                    allowed: false,
                    ts: now_ms(),
                })?;
                return Ok(ComputerExecution {
                    result: niucode_tools::ToolResult {
                        ok: false,
                        output: "computer action denied by user".into(),
                    },
                    images: Vec::new(),
                });
            }
            session.append(&Event::Approval {
                tool: "computer".into(),
                allowed: true,
                ts: now_ms(),
            })?;
        }
        PolicyDecision::Allowed => {}
    }

    if let ComputerAction::Observe { window_id } = action {
        let capture = {
            let mut runtime = runtime.lock().unwrap();
            runtime.observe(*window_id)
        };
        let capture = match capture {
            Ok(capture) => capture,
            Err(error) => {
                return Ok(ComputerExecution {
                    result: niucode_tools::ToolResult {
                        ok: false,
                        output: format!("computer observe failed: {error:#}"),
                    },
                    images: Vec::new(),
                });
            }
        };
        // The lock is released before this approval await.
        if capture.observation.window.kind == WindowKind::Sensitive {
            let summary = format!(
                "computer observe sensitive window {} — {}. Purpose: {}",
                clean_display(&capture.observation.window.app, 60),
                clean_display(&capture.observation.window.title, 80),
                clean_display(purpose, 200)
            );
            let image = approval_image(&capture.png);
            if !io
                .approve(
                    session,
                    &call.id,
                    ApprovalRequest {
                        tool: "computer".into(),
                        summary,
                        preview: None,
                        image,
                    },
                )
                .await?
            {
                session.append(&Event::Approval {
                    tool: "computer".into(),
                    allowed: false,
                    ts: now_ms(),
                })?;
                return Ok(ComputerExecution {
                    result: niucode_tools::ToolResult {
                        ok: false,
                        output: "sensitive-window observation denied by user".into(),
                    },
                    images: Vec::new(),
                });
            }
            session.append(&Event::Approval {
                tool: "computer".into(),
                allowed: true,
                ts: now_ms(),
            })?;
        }
        cap_visual_budget(messages, MAX_VISUAL_PARTS - 1);
        let encoded = base64::engine::general_purpose::STANDARD.encode(&capture.png);
        let data_url = format!("data:image/png;base64,{encoded}");
        let mut output = format!(
            "observation {} · window {} · {} — {} · {}x{} · expires_at_ms={}",
            capture.observation.id,
            capture.observation.window.id.0,
            capture.observation.window.app,
            capture.observation.window.title,
            capture.observation.image.width,
            capture.observation.image.height,
            capture.observation.expires_at_ms
        );
        if config.computer_keep_frames {
            let root = session
                .path()
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join("attachments");
            match AttachmentStore::open(root).and_then(|store| {
                store.put(
                    &capture.png,
                    Some(&format!("{}.png", capture.observation.id)),
                    Some("computer observation"),
                )
            }) {
                Ok(reference) => {
                    output.push_str(&format!(" · frame_kept sha256={}", reference.sha256));
                }
                Err(error) => {
                    output.push_str(&format!(" · frame_keep_failed={error}"));
                }
            }
        }
        let result = niucode_tools::ToolResult { ok: true, output };
        audit_computer_target(
            session,
            action,
            purpose,
            &result,
            Some(capture.observation.window.id),
            Some(capture.observation.id.to_string()),
        )?;
        return Ok(ComputerExecution {
            result,
            images: vec![data_url],
        });
    }

    let input = {
        let mut runtime = runtime.lock().unwrap();
        match runtime.validate(action, now_ms()) {
            Ok(observation) => runtime.perform(action, &observation),
            Err(error) => Err(anyhow::anyhow!(error.to_string())),
        }
    };
    match input {
        Ok(()) => Ok(ComputerExecution {
            result: niucode_tools::ToolResult {
                ok: true,
                output: format!("computer {} completed", action_label(action)),
            },
            images: Vec::new(),
        }),
        Err(error) => Ok(ComputerExecution {
            result: niucode_tools::ToolResult {
                ok: false,
                output: format!("computer action rejected: {error:#}"),
            },
            images: Vec::new(),
        }),
    }
}

const MAX_VISUAL_PARTS: usize = 4;
const MAX_APPROVAL_IMAGE: usize = 2_000_000;

fn approval_image(png: &[u8]) -> Option<String> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(png);
    (encoded.len() <= MAX_APPROVAL_IMAGE).then(|| format!("data:image/png;base64,{encoded}"))
}

/// Keep at most `max` image parts across the whole provider request. The
/// oldest images are replaced by a text marker; JSONL events are untouched.
fn cap_visual_budget(messages: &mut [ChatMessage], max: usize) {
    let mut positions = Vec::new();
    for (message_index, message) in messages.iter().enumerate() {
        if let Some(MessageContent::Parts(parts)) = &message.content {
            for (part_index, part) in parts.iter().enumerate() {
                if matches!(part, ContentPart::ImageUrl { .. }) {
                    positions.push((message_index, part_index));
                }
            }
        }
    }
    if positions.len() <= max {
        return;
    }
    let drop_count = positions.len() - max;
    for (message_index, part_index) in positions.into_iter().take(drop_count) {
        if let Some(MessageContent::Parts(parts)) = &mut messages[message_index].content {
            parts[part_index] = ContentPart::Text {
                text: "[image omitted: visual budget]".into(),
            };
        }
    }
}

fn clean_display(value: &str, max_chars: usize) -> String {
    let mut out: String = value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(max_chars)
        .collect();
    if value.chars().count() > max_chars {
        out.push('…');
    }
    out
}

fn action_detail(action: &ComputerAction) -> String {
    match action {
        ComputerAction::Observe { .. } => "observe".into(),
        ComputerAction::Click { point, button, .. } => {
            format!("click {button:?} at ({}, {})", point.x, point.y)
        }
        ComputerAction::Type { text, .. } => {
            format!("type {} character(s)", text.chars().count())
        }
        ComputerAction::Key { key, .. } => {
            let mut parts = key
                .modifiers
                .iter()
                .map(|modifier| format!("{modifier:?}"))
                .collect::<Vec<_>>();
            parts.push(clean_display(&key.key, 30));
            format!("key {}", parts.join("+"))
        }
        ComputerAction::Scroll {
            point,
            direction,
            amount,
            ..
        } => format!(
            "scroll {direction:?} {amount} at ({}, {})",
            point.x, point.y
        ),
    }
}

fn action_label(action: &ComputerAction) -> &'static str {
    match action {
        ComputerAction::Observe { .. } => "observe",
        ComputerAction::Click { .. } => "click",
        ComputerAction::Type { .. } => "type",
        ComputerAction::Scroll { .. } => "scroll",
        ComputerAction::Key { .. } => "key",
    }
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

fn bounded_json(value: &Value) -> String {
    let rendered = serde_json::to_string(value).unwrap_or_default();
    if rendered.len() <= 800 {
        rendered
    } else {
        let mut end = 800;
        while !rendered.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &rendered[..end])
    }
}

fn mcp_call_result(result: Result<Result<niucode_mcp::McpCall>, anyhow::Error>) -> ToolResult {
    match result {
        Ok(Ok(call)) => ToolResult {
            ok: call.ok,
            output: call.output,
        },
        Ok(Err(error)) | Err(error) => {
            let message = format!("mcp error: {error:#}");
            let output = if message.to_ascii_lowercase().contains("timed out") {
                format!(
                    "{message}; the mutation outcome is unknown, do not retry without human confirmation"
                )
            } else {
                message
            };
            ToolResult { ok: false, output }
        }
    }
}

async fn execute_builtin(
    session: &SessionLog,
    config: &AgentConfig,
    io: &mut runtime::RunIo<'_>,
    toolbox: &Toolbox,
    call: &ToolCall,
) -> Result<niucode_tools::RecordedToolResult> {
    let prepared = if matches!(call.name.as_str(), "write" | "edit") {
        match toolbox.prepare_mutation(&call.name, &call.input) {
            Ok(prepared) => Some(prepared),
            Err(error) => return Ok(niucode_tools::RecordedToolResult::failed(error)),
        }
    } else {
        None
    };
    if requires_approval(&call.name) && config.approval == ApprovalMode::Ask {
        let request = ApprovalRequest {
            tool: call.name.clone(),
            summary: approval_summary(call),
            preview: prepared.as_ref().map(|p| p.preview().clone()),
            image: None,
        };
        let allowed = io.approve(session, &call.id, request).await?;
        session.append(&Event::Approval {
            tool: call.name.clone(),
            allowed,
            ts: now_ms(),
        })?;
        if !allowed {
            return Ok(niucode_tools::RecordedToolResult::failed(anyhow::anyhow!(
                "approval denied by user"
            )));
        }
    }
    // Waiting for a human never holds W1. Prepared contents are revalidated
    // after acquiring it, so another writer cannot invalidate an approval silently.
    let _guard = if requires_approval(&call.name) {
        match &config.write_lock {
            Some(lock) => Some(io.wait(session, lock.lock()).await?),
            None => None,
        }
    } else {
        None
    };
    let result = if let Some(prepared) = prepared {
        toolbox.execute_prepared(prepared)
    } else {
        io.wait(session, toolbox.execute_recorded(&call.name, &call.input))
            .await?
    };
    if result.evidence != niucode_core::ExecutionEvidence::default() {
        session.append(&Event::ToolEvidence {
            call_id: call.id.clone(),
            evidence: result.evidence.clone(),
            ts: now_ms(),
        })?;
        if result.evidence.process.is_some() {
            // Keep W1 through the post-command snapshot; another internal writer
            // must not make unchecked contents appear to belong to this command.
            let snapshot = io
                .wait(
                    session,
                    changes::capture(&config.working_dir, session.path(), false),
                )
                .await?;
            session.append(&Event::CheckSnapshot {
                call_id: call.id.clone(),
                digest: changes::fingerprint(&snapshot),
                ts: now_ms(),
            })?;
        }
    }
    Ok(result)
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

fn cap_tool_output(output: String) -> String {
    let bytes = output.as_bytes();
    if bytes.len() <= MAX_TOOL_OUTPUT_BYTES {
        return output;
    }
    let mut head_end = TOOL_OUTPUT_HEAD_BYTES.min(bytes.len());
    while head_end > 0 && !output.is_char_boundary(head_end) {
        head_end -= 1;
    }
    let mut tail_start = bytes.len().saturating_sub(TOOL_OUTPUT_TAIL_BYTES);
    while tail_start < bytes.len() && !output.is_char_boundary(tail_start) {
        tail_start += 1;
    }
    let omitted = tail_start.saturating_sub(head_end);
    format!(
        "{}\n… truncated {omitted} bytes …\n{}",
        &output[..head_end],
        &output[tail_start..]
    )
}

fn retry_delay(error: &ProviderError, attempt: usize) -> Duration {
    error
        .retry_after()
        .unwrap_or_else(|| {
            Duration::from_millis(RETRY_BASE_MS.saturating_mul(1_u64 << (attempt - 1).min(5)))
        })
        .min(Duration::from_secs(MAX_RETRY_DELAY_SECS))
}

fn maybe_compact(
    session: &SessionLog,
    config: &AgentConfig,
    system_prompt: &str,
    messages: &mut Vec<ChatMessage>,
    tools: &[niucode_provider::ToolSpec],
    force: bool,
) -> Result<Option<AgentEvent>> {
    let before = estimate_context(messages, tools);
    if !force && !config.context.should_compact(before) {
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

/// Typed computer text is sensitive; the JSONL keeps length + SHA-256 rather
/// than the literal keystrokes. The live call still executes with the original.
fn redact_tool_calls_for_log(calls: &mut [ToolCall], log_input: bool) {
    if log_input {
        return;
    }
    for call in calls {
        if call.name != "computer"
            || call.input.get("action").and_then(Value::as_str) != Some("type")
        {
            continue;
        }
        let Some(text) = call.input.get("text").and_then(Value::as_str) else {
            continue;
        };
        let characters = text.chars().count();
        let digest: String = Sha256::digest(text.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        call.input["text"] =
            Value::String(format!("[redacted: {characters} chars sha256:{digest}]"));
        call.input["text_chars"] = Value::from(characters);
        call.input["text_sha256"] = Value::String(digest);
    }
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
                "task_started",
                "user_message",
                "task_snapshot",
                "assistant_message",
                "turn_end",
                "session_end",
                "task_snapshot",
                "task_finished"
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

    struct FakeComputer {
        performed: Vec<ComputerAction>,
        kind: WindowKind,
    }

    impl Default for FakeComputer {
        fn default() -> Self {
            Self {
                performed: Vec::new(),
                kind: WindowKind::Normal,
            }
        }
    }

    impl niucode_computer::ComputerBackend for FakeComputer {
        fn observe(
            &mut self,
            _window: Option<WindowId>,
        ) -> Result<niucode_computer::Capture, Box<dyn std::error::Error + Send + Sync>> {
            let now = now_ms();
            Ok(niucode_computer::Capture {
                observation: niucode_computer::Observation {
                    id: niucode_computer::ObservationId::new("obs-1"),
                    window: niucode_computer::WindowInfo {
                        id: WindowId(7),
                        app: "editor".into(),
                        title: "notes".into(),
                        class: None,
                        kind: self.kind,
                        bounds: niucode_computer::Rect {
                            x: 0,
                            y: 0,
                            width: 10,
                            height: 10,
                        },
                    },
                    image: niucode_computer::ImageMeta {
                        width: 10,
                        height: 10,
                        scale_percent: 100,
                        sha256: "abc".into(),
                        bytes: 3,
                    },
                    captured_at_ms: now,
                    expires_at_ms: now + 60_000,
                },
                png: b"\x89PNG\r\n\x1a\nimage".to_vec(),
            })
        }

        fn perform(
            &mut self,
            action: &ComputerAction,
            _observation: &niucode_computer::Observation,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.performed.push(action.clone());
            Ok(())
        }
    }

    #[tokio::test]
    async fn computer_observe_is_advertised_and_returns_an_image_part() {
        let directory = temp_dir("computer-observe");
        let mut session = SessionLog::create(&directory).expect("session");
        let mut config = test_config(&directory);
        let runtime = Arc::new(std::sync::Mutex::new(ComputerRuntime::new(
            Box::<FakeComputer>::default(),
            ComputerPolicy::Control,
        )));
        runtime.lock().unwrap().arm().expect("arm");
        config.computer = Some(runtime);
        let provider = ScriptedProvider::from_turns(vec![
            vec![
                StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 0,
                    id: Some("call_computer".into()),
                    name: Some("computer".into()),
                    arguments: r#"{"action":"observe","purpose":"inspect"}"#.to_string(),
                }),
                StreamEvent::Done {
                    finish_reason: Some("tool_calls".into()),
                },
            ],
            vec![
                StreamEvent::TextDelta("done".into()),
                StreamEvent::Done {
                    finish_reason: Some("stop".into()),
                },
            ],
        ]);
        run(
            &provider,
            &mut session,
            &config,
            "observe the desktop",
            &mut |_| true,
            &mut |_| {},
        )
        .await
        .expect("run");

        let requests = provider.requests();
        assert_eq!(requests.len(), 2);
        assert!(requests[0]
            .tools
            .iter()
            .any(|tool| tool.function.name == "computer"));
        let tool_message = requests[1]
            .messages
            .iter()
            .find(|message| message.role == Role::Tool)
            .expect("tool message");
        assert!(tool_message.has_images());
        let text = match tool_message.content.as_ref().expect("content") {
            niucode_provider::MessageContent::Parts(parts) => parts
                .iter()
                .find_map(|part| match part {
                    niucode_provider::ContentPart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .expect("text part"),
            niucode_provider::MessageContent::Text(text) => text,
        };
        assert!(text.contains("observation obs-1"));
        assert!(
            !directory.join("attachments").exists(),
            "frames must be opt-in"
        );
    }

    #[derive(Clone, Default)]
    struct StopFlag(std::sync::Arc<std::sync::atomic::AtomicBool>);

    struct StopBackend {
        flag: StopFlag,
    }

    impl niucode_computer::ComputerBackend for StopBackend {
        fn observe(
            &mut self,
            _window: Option<WindowId>,
        ) -> Result<niucode_computer::Capture, Box<dyn std::error::Error + Send + Sync>> {
            Err("unused".into())
        }

        fn perform(
            &mut self,
            _action: &ComputerAction,
            _observation: &niucode_computer::Observation,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        fn emergency_stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.flag.0.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn visual_budget_drops_the_oldest_images_first() {
        let mut messages = vec![
            ChatMessage::user_with_images("one", vec!["a".into()]),
            ChatMessage::user_with_images("two", vec!["b".into()]),
            ChatMessage::user_with_images("three", vec!["c".into()]),
        ];
        cap_visual_budget(&mut messages, 2);
        assert_eq!(
            messages
                .iter()
                .filter(|message| message.has_images())
                .count(),
            2
        );
        let first = match messages[0].content.as_ref().unwrap() {
            MessageContent::Parts(parts) => parts,
            MessageContent::Text(_) => panic!("parts expected"),
        };
        assert!(first.iter().any(|part| matches!(
            part,
            ContentPart::Text { text } if text.contains("visual budget")
        )));
        assert!(messages[1].has_images());
        assert!(messages[2].has_images());
    }

    #[tokio::test]
    async fn provider_request_never_exceeds_four_images() {
        let directory = temp_dir("visual-budget-request");
        let mut session = SessionLog::create(&directory).expect("session");
        let config = test_config(&directory);
        let history = (0..5)
            .map(|index| {
                ChatMessage::user_with_images(
                    format!("image {index}"),
                    vec![format!("data:image/png;base64,{index}")],
                )
            })
            .collect::<Vec<_>>();
        let provider = ScriptedProvider::from_turns(vec![vec![
            StreamEvent::TextDelta("done".into()),
            StreamEvent::Done {
                finish_reason: Some("stop".into()),
            },
        ]]);
        run_with_history(
            &provider,
            &mut session,
            &config,
            "continue",
            history,
            &mut |_| true,
            &mut |_| {},
        )
        .await
        .expect("run");
        let requests = provider.requests();
        let images = requests[0]
            .messages
            .iter()
            .map(|message| match message.content.as_ref() {
                Some(MessageContent::Parts(parts)) => parts
                    .iter()
                    .filter(|part| matches!(part, ContentPart::ImageUrl { .. }))
                    .count(),
                _ => 0,
            })
            .sum::<usize>();
        assert!(images <= 4, "request carried {images} images");
    }

    #[test]
    fn log_input_opt_in_keeps_typed_text() {
        let call = || ToolCall {
            id: "type".into(),
            name: "computer".into(),
            input: json!({"action": "type", "text": "hunter2"}),
        };
        let mut redacted = vec![call()];
        redact_tool_calls_for_log(&mut redacted, false);
        assert!(!redacted[0].input["text"]
            .as_str()
            .unwrap()
            .contains("hunter2"));
        let mut plain = vec![call()];
        redact_tool_calls_for_log(&mut plain, true);
        assert_eq!(plain[0].input["text"], "hunter2");
    }

    #[test]
    fn approval_display_cleans_control_chars_and_truncates() {
        assert_eq!(clean_display("a\nb\tc", 10), "a b c");
        assert_eq!(clean_display("abcdef", 3), "abc…");
        assert_eq!(clean_display("ok", 10), "ok");
    }

    #[test]
    fn emergency_stop_disarms_and_releases_input() {
        let flag = StopFlag::default();
        let mut runtime = ComputerRuntime::new(
            Box::new(StopBackend { flag: flag.clone() }),
            ComputerPolicy::Control,
        );
        runtime.arm().expect("arm");
        runtime.emergency_stop().expect("stop");
        assert_eq!(runtime.policy(), ComputerPolicy::Off);
        assert!(!runtime.is_armed());
        assert!(flag.0.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn sensitive_observe_is_denied_without_approval() {
        let directory = temp_dir("sensitive-observe");
        let mut session = SessionLog::create(&directory).expect("session");
        let mut config = test_config(&directory);
        let fake = FakeComputer {
            kind: WindowKind::Sensitive,
            ..FakeComputer::default()
        };
        let runtime = Arc::new(std::sync::Mutex::new(ComputerRuntime::new(
            Box::new(fake),
            ComputerPolicy::Observe,
        )));
        runtime.lock().unwrap().arm().expect("arm");
        config.computer = Some(runtime);
        let provider = ScriptedProvider::from_turns(vec![
            vec![
                StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 0,
                    id: Some("observe".into()),
                    name: Some("computer".into()),
                    arguments: r#"{"action":"observe"}"#.to_string(),
                }),
                StreamEvent::Done {
                    finish_reason: Some("tool_calls".into()),
                },
            ],
            vec![
                StreamEvent::TextDelta("done".into()),
                StreamEvent::Done {
                    finish_reason: Some("stop".into()),
                },
            ],
        ]);
        let mut approvals = 0;
        run(
            &provider,
            &mut session,
            &config,
            "observe the password window",
            &mut |_| {
                approvals += 1;
                false
            },
            &mut |_| {},
        )
        .await
        .expect("run");
        assert_eq!(approvals, 1);
        let requests = provider.requests();
        let tool = requests[1]
            .messages
            .iter()
            .rev()
            .find(|message| message.role == Role::Tool)
            .expect("tool result");
        assert!(!tool.has_images());
        assert!(tool.text().unwrap_or_default().contains("denied"));
        let loaded = niucode_core::load(session.path()).expect("load");
        assert!(loaded.events.iter().any(|event| matches!(
            event,
            Event::Approval { tool, allowed: false, .. } if tool == "computer"
        )));
    }

    #[tokio::test]
    async fn sensitive_observe_is_returned_after_approval() {
        let directory = temp_dir("sensitive-observe-allow");
        let mut session = SessionLog::create(&directory).expect("session");
        let mut config = test_config(&directory);
        let fake = FakeComputer {
            kind: WindowKind::Sensitive,
            ..FakeComputer::default()
        };
        let runtime = Arc::new(std::sync::Mutex::new(ComputerRuntime::new(
            Box::new(fake),
            ComputerPolicy::Observe,
        )));
        runtime.lock().unwrap().arm().expect("arm");
        config.computer = Some(runtime);
        let provider = ScriptedProvider::from_turns(vec![
            vec![
                StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 0,
                    id: Some("observe".into()),
                    name: Some("computer".into()),
                    arguments: r#"{"action":"observe"}"#.to_string(),
                }),
                StreamEvent::Done {
                    finish_reason: Some("tool_calls".into()),
                },
            ],
            vec![
                StreamEvent::TextDelta("done".into()),
                StreamEvent::Done {
                    finish_reason: Some("stop".into()),
                },
            ],
        ]);
        let mut approvals = 0;
        run(
            &provider,
            &mut session,
            &config,
            "observe the password window",
            &mut |request| {
                approvals += 1;
                assert!(
                    request.image.is_some(),
                    "approval should carry the screenshot"
                );
                true
            },
            &mut |_| {},
        )
        .await
        .expect("run");
        assert_eq!(approvals, 1);
        let requests = provider.requests();
        let tool = requests[1]
            .messages
            .iter()
            .rev()
            .find(|message| message.role == Role::Tool)
            .expect("tool result");
        assert!(tool.has_images());
        let loaded = niucode_core::load(session.path()).expect("load");
        assert!(loaded.events.iter().any(|event| matches!(
            event,
            Event::Approval { tool, allowed: true, .. } if tool == "computer"
        )));
    }

    #[tokio::test]
    async fn computer_observe_can_keep_a_frame() {
        let directory = temp_dir("computer-frame");
        let mut session = SessionLog::create(&directory).expect("session");
        let mut config = test_config(&directory);
        config.computer_keep_frames = true;
        let runtime = Arc::new(std::sync::Mutex::new(ComputerRuntime::new(
            Box::<FakeComputer>::default(),
            ComputerPolicy::Observe,
        )));
        runtime.lock().unwrap().arm().expect("arm");
        config.computer = Some(runtime);
        let provider = ScriptedProvider::from_turns(vec![
            vec![
                StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 0,
                    id: Some("observe".into()),
                    name: Some("computer".into()),
                    arguments: r#"{"action":"observe"}"#.to_string(),
                }),
                StreamEvent::Done {
                    finish_reason: Some("tool_calls".into()),
                },
            ],
            vec![
                StreamEvent::TextDelta("done".into()),
                StreamEvent::Done {
                    finish_reason: Some("stop".into()),
                },
            ],
        ]);
        run(
            &provider,
            &mut session,
            &config,
            "observe",
            &mut |_| true,
            &mut |_| {},
        )
        .await
        .expect("run");
        let attachments = directory.join("attachments");
        assert!(
            attachments.read_dir().unwrap().next().is_some(),
            "kept frame was not stored"
        );
        let loaded = niucode_core::load(session.path()).expect("load");
        assert!(loaded.events.iter().any(|event| matches!(
            event,
            Event::ToolResult { output, .. } if output.contains("frame_kept sha256=")
        )));
    }

    #[tokio::test]
    async fn computer_input_requires_and_records_approval() {
        let directory = temp_dir("computer-input");
        let mut session = SessionLog::create(&directory).expect("session");
        let mut config = test_config(&directory);
        let runtime = Arc::new(std::sync::Mutex::new(ComputerRuntime::new(
            Box::<FakeComputer>::default(),
            ComputerPolicy::Control,
        )));
        runtime.lock().unwrap().arm().expect("arm");
        config.computer = Some(runtime);
        let provider = ScriptedProvider::from_turns(vec![
            vec![
                StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 0,
                    id: Some("observe".into()),
                    name: Some("computer".into()),
                    arguments: r#"{"action":"observe"}"#.to_string(),
                }),
                StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 1,
                    id: Some("click".into()),
                    name: Some("computer".into()),
                    arguments:
                        r#"{"action":"click","observation_id":"obs-1","window_id":7,"x":5,"y":5}"#
                            .to_string(),
                }),
                StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 2,
                    id: Some("type".into()),
                    name: Some("computer".into()),
                    arguments:
                        r#"{"action":"type","observation_id":"obs-1","window_id":7,"text":"hunter2"}"#
                            .to_string(),
                }),
                StreamEvent::Done {
                    finish_reason: Some("tool_calls".into()),
                },
            ],
            vec![
                StreamEvent::TextDelta("done".into()),
                StreamEvent::Done {
                    finish_reason: Some("stop".into()),
                },
            ],
        ]);
        let mut approvals = 0;
        let mut summaries = Vec::new();
        run(
            &provider,
            &mut session,
            &config,
            "click the window",
            &mut |request| {
                approvals += 1;
                assert!(
                    request.image.is_some(),
                    "approval should carry the target frame"
                );
                summaries.push(request.summary.clone());
                true
            },
            &mut |_| {},
        )
        .await
        .expect("run");
        assert_eq!(approvals, 2);
        assert!(summaries
            .iter()
            .any(|summary| summary.contains("click Left at (5, 5)")));
        assert!(summaries
            .iter()
            .any(|summary| summary.contains("type 7 character(s)")));
        assert!(!summaries.iter().any(|summary| summary.contains("hunter2")));
        let loaded = niucode_core::load(session.path()).expect("load");
        assert!(loaded.events.iter().any(|event| matches!(
            event,
            Event::Approval { tool, allowed: true, .. } if tool == "computer"
        )));
        assert!(loaded.events.iter().any(|event| matches!(
            event,
            Event::ToolResult { call_id, ok: true, .. } if call_id == "click"
        )));
        let typed = loaded
            .events
            .iter()
            .find_map(|event| match event {
                Event::AssistantMessage { tool_calls, .. } => tool_calls
                    .iter()
                    .find(|call| call.input.get("action").and_then(Value::as_str) == Some("type")),
                _ => None,
            })
            .expect("typed call");
        let logged = typed
            .input
            .get("text")
            .and_then(Value::as_str)
            .expect("text");
        assert!(
            !logged.contains("hunter2"),
            "plaintext typed text leaked: {logged}"
        );
        assert_eq!(
            typed.input.get("text_chars").and_then(Value::as_u64),
            Some(7)
        );
        assert_eq!(
            typed
                .input
                .get("text_sha256")
                .and_then(Value::as_str)
                .map(str::len),
            Some(64)
        );
        assert!(!loaded.events.iter().any(|event| matches!(
            event,
            Event::ToolResult { output, .. } if output.contains("hunter2")
        )));
        let typed_audit = loaded
            .events
            .iter()
            .find_map(|event| match event {
                Event::ComputerAction {
                    action,
                    text_chars,
                    text_sha256,
                    purpose,
                    ..
                } if action == "type" => Some((*text_chars, text_sha256.clone(), purpose.clone())),
                _ => None,
            })
            .expect("typed audit");
        assert_eq!(typed_audit.0, 7);
        assert_eq!(typed_audit.1.as_deref().map(str::len), Some(64));
        assert_eq!(typed_audit.2, "no purpose provided");
        assert!(loaded.events.iter().any(|event| matches!(
            event,
            Event::ComputerAction { action, ok: true, .. } if action == "click"
        )));
    }

    #[tokio::test]
    async fn attachments_are_stored_budgeted_and_sent_as_parts() {
        let directory = temp_dir("attachments");
        let store = AttachmentStore::open(directory.join("attachments")).expect("store");
        let image = store
            .put(b"\x89PNG\r\n\x1a\nimage", Some("shot.png"), Some("screen"))
            .expect("image");
        let script = store
            .put(b"print('hi')\n", Some("script.py"), None)
            .expect("script");
        let mut session = SessionLog::create(&directory).expect("session");
        let provider = ScriptedProvider::from_turns(vec![vec![
            StreamEvent::TextDelta("done".into()),
            StreamEvent::Done {
                finish_reason: Some("stop".into()),
            },
        ]]);
        let config = test_config(&directory);
        let outcome = run_with_attachments(
            &provider,
            &mut session,
            &config,
            "look at this",
            vec![image, script],
            &store,
            &mut |_| true,
            &mut |_| {},
        )
        .await
        .expect("run");
        assert_eq!(outcome.final_text, "done");

        let request = provider.requests().remove(0);
        let user = request.messages.last().expect("user message");
        assert!(user.has_images());
        let parts = match user.content.as_ref().expect("content") {
            niucode_provider::MessageContent::Parts(parts) => parts,
            _ => panic!("expected parts"),
        };
        assert!(parts.iter().any(|part| matches!(
            part,
            niucode_provider::ContentPart::ImageUrl { image_url }
                if image_url.url.starts_with("data:image/png;base64,")
        )));
        assert!(parts.iter().any(|part| matches!(
            part,
            niucode_provider::ContentPart::Text { text }
                if text.contains("<attachment name=\"script.py\">")
                    && text.contains("print('hi')")
        )));

        let loaded = load(session.path()).expect("load");
        let attachments = loaded
            .events
            .iter()
            .find_map(|event| match event {
                Event::UserMessage { attachments, .. } => Some(attachments.clone()),
                _ => None,
            })
            .expect("user event");
        assert_eq!(attachments.len(), 2);
    }

    #[test]
    fn projects_conversation_history_from_events() {
        let events = vec![
            Event::UserMessage {
                text: "first".into(),
                attachments: Vec::new(),
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
                finish_reason: None,
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
                attachments: Vec::new(),
                ts: 3,
            },
        ];

        let history = project_history(&events).expect("project history");
        assert_eq!(history.len(), 3);
        assert_eq!(history[1].role, Role::Tool);
        assert!(history[1]
            .text()
            .is_some_and(|content| content.contains("outcome unknown")));
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
            .any(|message| message.text() == Some("first task")));

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
    fn caps_tool_output_with_head_and_tail() {
        let input = format!(
            "{}{}{}",
            "a".repeat(200_000),
            "b".repeat(1_200_000),
            "c".repeat(200_000)
        );
        let output = cap_tool_output(input);
        assert!(output.len() < 1_048_576 + 1_000);
        assert!(output.starts_with("a"));
        assert!(output.ends_with('c'));
        assert!(output.contains("truncated"));
    }

    #[test]
    fn mcp_timeout_result_tells_the_model_not_to_retry() {
        let result = mcp_call_result(Ok(Err(anyhow::anyhow!("MCP response timed out"))));
        assert!(!result.ok);
        assert!(
            result.output.contains("outcome is unknown"),
            "{}",
            result.output
        );
        assert!(result.output.contains("do not retry"), "{}", result.output);
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
