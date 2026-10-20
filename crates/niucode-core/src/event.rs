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
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    SessionStart {
        id: String,
        cwd: String,
        model: String,
        ts: Timestamp,
    },
    UserMessage {
        text: String,
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
    /// Active session goal / completion condition.
    GoalSet {
        text: String,
        ts: Timestamp,
    },
    /// Active session goal was cleared.
    GoalCleared {
        ts: Timestamp,
    },
    /// A sidecar extension was mounted for this session.
    ExtensionMount {
        name: String,
        version: String,
        tools: Vec<String>,
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
    /// Layered AGENTS.md files loaded for this run.
    Instructions {
        #[serde(default)]
        sources: Vec<String>,
        #[serde(default)]
        digest: String,
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
            Self::GoalSet { .. } => "goal_set",
            Self::GoalCleared { .. } => "goal_cleared",
            Self::ExtensionMount { .. } => "extension_mount",
            Self::ExtensionUnmount { .. } => "extension_unmount",
            Self::ExtensionError { .. } => "extension_error",
            Self::Instructions { .. } => "instructions",
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
            | Self::GoalSet { ts, .. }
            | Self::GoalCleared { ts }
            | Self::ExtensionMount { ts, .. }
            | Self::ExtensionUnmount { ts, .. }
            | Self::ExtensionError { ts, .. }
            | Self::Instructions { ts, .. }
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
            session: "sub.jsonl".into(),
            max_steps: 6,
            timeout_secs: 120,
            context_budget: 16_000,
            ts: 4,
        };
        assert_eq!(spawn.kind(), "subagent_spawn");
        assert_eq!(spawn.ts(), 4);
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
}
