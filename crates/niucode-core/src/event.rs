//! Durable session events.
//!
//! Events are the only representation of a session that survives the process.
//! The in-memory conversation is a projection of these events; if something is
//! not appended here, it does not exist after the process exits.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Milliseconds since the Unix epoch.
pub type Timestamp = u64;

/// Stable, positional event identifier.
///
/// The log is append-only, so the ordinal of a decoded event never changes.
/// Corrupt lines are skipped by `load`, and ordinals count decoded events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(u64);

impl EventId {
    pub fn from_index(index: usize) -> Self {
        Self(index as u64 + 1)
    }

    /// Zero-based index into the decoded event list.
    pub fn index(self) -> usize {
        self.0.saturating_sub(1) as usize
    }

    pub fn parse(input: &str) -> Result<Self, String> {
        let input = input.trim();
        if input.is_empty() {
            return Err("event id is empty; use e000012".to_string());
        }
        let digits = input
            .strip_prefix('e')
            .or_else(|| input.strip_prefix('E'))
            .unwrap_or(input);
        if digits.is_empty() || !digits.chars().all(|character| character.is_ascii_digit()) {
            return Err(format!("invalid event id {input:?}; use e000012"));
        }
        let value: u64 = digits
            .parse()
            .map_err(|_| format!("event id {input:?} is out of range"))?;
        if value == 0 {
            return Err("event ids are 1-based; e000000 is invalid".to_string());
        }
        Ok(Self(value))
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "e{:06}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cached_tokens: u64,
    #[serde(default)]
    pub reasoning_tokens: u64,
}

impl Usage {
    /// Cached input tokens as a percentage of all input tokens.
    pub fn cache_hit_percent(&self) -> u64 {
        self.cached_tokens
            .saturating_mul(100)
            .checked_div(self.input_tokens)
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProcessOutcome {
    Exited { code: i32 },
    Signaled { signal: Option<i32> },
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub before_digest: Option<String>,
    pub after_digest: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExecutionEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<ProcessOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<FileChange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub path: String,
    pub digest: Option<String>,
    pub text: Option<String>,
    pub mode: Option<u32>,
    pub note: Option<String>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub files: Vec<FileSnapshot>,
    pub complete: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    SessionStart {
        id: String,
        cwd: String,
        model: String,
        #[serde(default)]
        provider: String,
        ts: Timestamp,
    },
    UserMessage {
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        attachments: Vec<crate::AttachmentRef>,
        ts: Timestamp,
    },
    AssistantMessage {
        #[serde(default)]
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reasoning: Option<String>,
        #[serde(default)]
        tool_calls: Vec<ToolCall>,
        ts: Timestamp,
    },
    ToolResult {
        call_id: String,
        ok: bool,
        output: String,
        ts: Timestamp,
    },
    TurnEnd {
        #[serde(default)]
        usage: Usage,
        /// Provider termination reason, absent in older session logs.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        finish_reason: Option<String>,
        ts: Timestamp,
    },
    Compaction {
        summary: String,
        /// Absolute event index replaced by this summary: `events[..upto]`.
        ///
        /// The compaction event itself is appended at the end of the log; the
        /// projection inserts its summary at this cutoff.
        upto: u64,
        /// Approximate context size before compaction, in tokens.
        #[serde(default)]
        before_tokens: u64,
        /// Approximate context size after compaction, in tokens.
        #[serde(default)]
        after_tokens: u64,
        ts: Timestamp,
    },
    /// Effective sandbox/approval/network policy for this run.
    Policy {
        sandbox: String,
        approval: String,
        network: String,
        backend: String,
        ts: Timestamp,
    },
    /// Human approval decision for a mutating tool call.
    Approval {
        tool: String,
        allowed: bool,
        ts: Timestamp,
    },
    /// Provenance for a session created by `niucode fork`.
    Fork {
        parent: String,
        parent_event: String,
        ts: Timestamp,
    },
    /// A read-only subagent was spawned from this session.
    SubagentSpawn {
        id: String,
        call_id: String,
        task: String,
        #[serde(default)]
        profile: String,
        session: String,
        max_steps: u64,
        timeout_secs: u64,
        context_budget: u64,
        ts: Timestamp,
    },
    /// A subagent finished, failed, or timed out.
    SubagentResult {
        id: String,
        call_id: String,
        ok: bool,
        summary: String,
        session: String,
        turns: u64,
        #[serde(default)]
        usage: Usage,
        ts: Timestamp,
    },
    /// Plan mode was enabled or disabled for the session.
    PlanMode {
        active: bool,
        ts: Timestamp,
    },
    /// A human-run verification command and its recorded result.
    Verification {
        command: String,
        ok: bool,
        /// Bounded stdout/stderr tail; never the full output.
        #[serde(default)]
        output: String,
        /// Optional niuse plan step, e.g. `p-2`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        step: Option<String>,
        ts: Timestamp,
    },
    /// Active session goal / completion condition.
    GoalSet {
        text: String,
        ts: Timestamp,
    },
    /// Active session goal was cleared.
    GoalCleared {
        ts: Timestamp,
    },
    /// Evaluator verdict for a /goal run turn.
    GoalVerdict {
        status: String,
        reason: String,
        ts: Timestamp,
    },
    /// Runtime model selection changed without starting a new session.
    ModelChange {
        #[serde(default)]
        provider: String,
        model: String,
        ts: Timestamp,
    },
    /// Runtime permission profile changed without starting a new session.
    PermissionChange {
        profile: String,
        sandbox: String,
        approval: String,
        network: String,
        ts: Timestamp,
    },
    /// A sidecar extension was mounted for this session.
    ExtensionMount {
        name: String,
        version: String,
        tools: Vec<String>,
        #[serde(default)]
        capabilities: Vec<String>,
        #[serde(default)]
        digest: String,
        #[serde(default)]
        workers: u64,
        ts: Timestamp,
    },
    /// An MCP server was mounted. Records the config hash and the mutating
    /// tool allowlist, not the environment values.
    McpMount {
        server: String,
        config_hash: String,
        #[serde(default)]
        write_tools: Vec<String>,
        ts: Timestamp,
    },
    /// A sidecar extension was unmounted.
    ExtensionUnmount {
        name: String,
        ts: Timestamp,
    },
    /// A sidecar extension failed to mount or answer.
    ExtensionError {
        name: String,
        message: String,
        ts: Timestamp,
    },
    /// Full `/init` proposal content; applying it is an explicit follow-up.
    InitProposal {
        content: String,
        path: String,
        ts: Timestamp,
    },
    /// Layered AGENTS.md files loaded for this run.
    Instructions {
        #[serde(default)]
        sources: Vec<String>,
        #[serde(default)]
        digest: String,
        ts: Timestamp,
    },
    TaskSnapshot {
        task_id: String,
        phase: String,
        snapshot: WorkspaceSnapshot,
        ts: Timestamp,
    },
    CheckSnapshot {
        call_id: String,
        digest: Option<String>,
        ts: Timestamp,
    },
    ToolEvidence {
        call_id: String,
        evidence: ExecutionEvidence,
        ts: Timestamp,
    },
    /// Accepted input is durable even while a provider or tool is blocked.
    InputSubmitted {
        id: String,
        text: String,
        queued: bool,
        ts: Timestamp,
    },
    /// The input was included at a safe model-request boundary.
    InputApplied {
        id: String,
        text: String,
        ts: Timestamp,
    },
    TaskStarted {
        id: String,
        #[serde(default)]
        cwd: String,
        ts: Timestamp,
    },
    TaskFinished {
        id: String,
        status: String,
        ts: Timestamp,
    },
    SessionEnd {
        ts: Timestamp,
    },
}

impl Event {
    /// Stable name used in logs and tests.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::SessionStart { .. } => "session_start",
            Self::UserMessage { .. } => "user_message",
            Self::AssistantMessage { .. } => "assistant_message",
            Self::ToolResult { .. } => "tool_result",
            Self::TurnEnd { .. } => "turn_end",
            Self::Compaction { .. } => "compaction",
            Self::Policy { .. } => "policy",
            Self::Approval { .. } => "approval",
            Self::Fork { .. } => "fork",
            Self::SubagentSpawn { .. } => "subagent_spawn",
            Self::SubagentResult { .. } => "subagent_result",
            Self::PlanMode { .. } => "plan_mode",
            Self::Verification { .. } => "verification",
            Self::GoalSet { .. } => "goal_set",
            Self::GoalCleared { .. } => "goal_cleared",
            Self::GoalVerdict { .. } => "goal_verdict",
            Self::ModelChange { .. } => "model_change",
            Self::PermissionChange { .. } => "permission_change",
            Self::ExtensionMount { .. } => "extension_mount",
            Self::McpMount { .. } => "mcp_mount",
            Self::ExtensionUnmount { .. } => "extension_unmount",
            Self::ExtensionError { .. } => "extension_error",
            Self::InitProposal { .. } => "init_proposal",
            Self::Instructions { .. } => "instructions",
            Self::TaskSnapshot { .. } => "task_snapshot",
            Self::CheckSnapshot { .. } => "check_snapshot",
            Self::ToolEvidence { .. } => "tool_evidence",
            Self::InputSubmitted { .. } => "input_submitted",
            Self::InputApplied { .. } => "input_applied",
            Self::TaskStarted { .. } => "task_started",
            Self::TaskFinished { .. } => "task_finished",
            Self::SessionEnd { .. } => "session_end",
        }
    }

    pub fn ts(&self) -> Timestamp {
        match self {
            Self::SessionStart { ts, .. }
            | Self::UserMessage { ts, .. }
            | Self::AssistantMessage { ts, .. }
            | Self::ToolResult { ts, .. }
            | Self::TurnEnd { ts, .. }
            | Self::Compaction { ts, .. }
            | Self::Policy { ts, .. }
            | Self::Approval { ts, .. }
            | Self::Fork { ts, .. }
            | Self::SubagentSpawn { ts, .. }
            | Self::SubagentResult { ts, .. }
            | Self::PlanMode { ts, .. }
            | Self::Verification { ts, .. }
            | Self::GoalSet { ts, .. }
            | Self::GoalCleared { ts }
            | Self::GoalVerdict { ts, .. }
            | Self::ModelChange { ts, .. }
            | Self::PermissionChange { ts, .. }
            | Self::ExtensionMount { ts, .. }
            | Self::McpMount { ts, .. }
            | Self::ExtensionUnmount { ts, .. }
            | Self::ExtensionError { ts, .. }
            | Self::Instructions { ts, .. }
            | Self::InitProposal { ts, .. }
            | Self::TaskSnapshot { ts, .. }
            | Self::CheckSnapshot { ts, .. }
            | Self::ToolEvidence { ts, .. }
            | Self::InputSubmitted { ts, .. }
            | Self::InputApplied { ts, .. }
            | Self::TaskStarted { ts, .. }
            | Self::TaskFinished { ts, .. }
            | Self::SessionEnd { ts } => *ts,
        }
    }
}

pub fn now_ms() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as Timestamp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_hit_percent_handles_zero_and_hits() {
        assert_eq!(Usage::default().cache_hit_percent(), 0);
        assert_eq!(
            Usage {
                input_tokens: 200,
                cached_tokens: 50,
                ..Usage::default()
            }
            .cache_hit_percent(),
            25
        );
    }

    #[test]
    fn event_ids_are_stable_and_parseable() {
        let id = EventId::from_index(11);
        assert_eq!(id.to_string(), "e000012");
        assert_eq!(EventId::parse("e000012"), Ok(id));
        assert_eq!(EventId::parse("12"), Ok(id));
        assert_eq!(EventId::parse("E12"), Ok(id));
        assert!(EventId::parse("e000000").is_err());
        assert!(EventId::parse("event").is_err());
        assert_eq!(id.index(), 11);
    }

    #[test]
    fn subagent_events_have_stable_kinds() {
        let spawn = Event::SubagentSpawn {
            id: "call_1.1".into(),
            call_id: "call_1".into(),
            task: "inspect".into(),
            profile: "explore".into(),
            session: "sub.jsonl".into(),
            max_steps: 6,
            timeout_secs: 120,
            context_budget: 16_000,
            ts: 4,
        };
        assert_eq!(spawn.kind(), "subagent_spawn");
        assert_eq!(spawn.ts(), 4);

        let model = Event::ModelChange {
            provider: "deepseek".into(),
            model: "deepseek-pro".into(),
            ts: 5,
        };
        assert_eq!(model.kind(), "model_change");
        assert_eq!(model.ts(), 5);
        let permission = Event::PermissionChange {
            profile: "review".into(),
            sandbox: "workspace-write".into(),
            approval: "on-failure".into(),
            network: "off".into(),
            ts: 6,
        };
        assert_eq!(permission.kind(), "permission_change");
        assert_eq!(permission.ts(), 6);
        let result = Event::SubagentResult {
            id: "call_1.1".into(),
            call_id: "call_1".into(),
            ok: true,
            summary: "found it".into(),
            session: "sub.jsonl".into(),
            turns: 2,
            usage: Usage::default(),
            ts: 5,
        };
        assert_eq!(result.kind(), "subagent_result");
        assert_eq!(result.ts(), 5);
    }

    #[test]
    fn fork_events_have_a_kind_and_timestamp() {
        let event = Event::Fork {
            parent: "/tmp/parent.jsonl".into(),
            parent_event: "e000003".into(),
            ts: 9,
        };
        assert_eq!(event.kind(), "fork");
        assert_eq!(event.ts(), 9);
    }

    #[test]
    fn instructions_have_a_stable_kind_and_timestamp() {
        let event = Event::Instructions {
            sources: vec!["/repo/AGENTS.md".into()],
            digest: "abc".into(),
            ts: 7,
        };
        assert_eq!(event.kind(), "instructions");
        assert_eq!(event.ts(), 7);
    }

    #[test]
    fn verification_event_roundtrips() {
        let event = Event::Verification {
            command: "cargo test".into(),
            ok: false,
            output: "one failure".into(),
            step: Some("p-2".into()),
            ts: 11,
        };
        assert_eq!(event.kind(), "verification");
        assert_eq!(event.ts(), 11);
        let encoded = serde_json::to_value(&event).unwrap();
        assert_eq!(encoded["type"], "verification");
        let decoded: Event = serde_json::from_value(encoded).unwrap();
        assert!(matches!(
            decoded,
            Event::Verification { command, ok: false, output, step: Some(step), ts: 11 }
                if command == "cargo test" && output == "one failure" && step == "p-2"
        ));
    }

    #[test]
    fn mcp_mount_event_roundtrips() {
        let event = Event::McpMount {
            server: "notes".into(),
            config_hash: "0123456789abcdef".into(),
            write_tools: vec!["mcp__notes__create_note".into()],
            ts: 12,
        };
        assert_eq!(event.kind(), "mcp_mount");
        assert_eq!(event.ts(), 12);
        let encoded = serde_json::to_value(&event).unwrap();
        assert_eq!(encoded["type"], "mcp_mount");
        let decoded: Event = serde_json::from_value(encoded).unwrap();
        assert!(matches!(
            decoded,
            Event::McpMount { server, config_hash, write_tools, ts: 12 }
                if server == "notes"
                    && config_hash == "0123456789abcdef"
                    && write_tools == ["mcp__notes__create_note"]
        ));
    }

    #[test]
    fn old_user_messages_decode_without_attachments() {
        let old = serde_json::json!({"type": "user_message", "text": "hello", "ts": 1});
        let event: Event = serde_json::from_value(old).unwrap();
        assert!(matches!(
            &event,
            Event::UserMessage {
                attachments,
                ..
            } if attachments.is_empty()
        ));
        let encoded = serde_json::to_value(event).unwrap();
        assert!(encoded.get("attachments").is_none());
    }

    #[test]
    fn user_attachments_roundtrip_with_metadata_only() {
        let event = Event::UserMessage {
            text: "look".into(),
            attachments: vec![crate::AttachmentRef {
                sha256: "a".repeat(64),
                media_type: "image/png".into(),
                kind: crate::AttachmentKind::Image,
                bytes: 42,
                text_chars: 0,
                name: Some("shot.png".into()),
                alt: Some("desktop".into()),
            }],
            ts: 3,
        };
        let encoded = serde_json::to_value(&event).unwrap();
        assert_eq!(
            encoded["attachments"][0]["sha256"].as_str().unwrap().len(),
            64
        );
        assert!(encoded["attachments"][0].get("data").is_none());
        let decoded: Event = serde_json::from_value(encoded).unwrap();
        assert!(
            matches!(decoded, Event::UserMessage { attachments, .. } if attachments.len() == 1)
        );
    }

    #[test]
    fn old_turn_end_logs_remain_readable_without_finish_reason() {
        let old = serde_json::json!({"type": "turn_end", "usage": {}, "ts": 1});
        let event: Event = serde_json::from_value(old).unwrap();
        assert!(matches!(
            &event,
            Event::TurnEnd {
                finish_reason: None,
                ..
            }
        ));
        assert!(serde_json::to_value(event)
            .unwrap()
            .get("finish_reason")
            .is_none());
    }
}
