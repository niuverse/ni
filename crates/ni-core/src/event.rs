//! Durable session events.
//!
//! Events are the only representation of a session that survives the process.
//! The in-memory conversation is a projection of these events; if something is
//! not appended here, it does not exist after the process exits.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Milliseconds since the Unix epoch.
pub type Timestamp = u64;

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
        /// Number of events replaced by this compaction.
        upto: u64,
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
