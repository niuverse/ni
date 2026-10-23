//! Core primitives for niucode.
//!
//! The crate intentionally contains no provider, tool, or UI code. It owns the
//! event log and the projection rules that everything else builds on.

pub mod attachment;
pub mod event;
pub mod session;

pub use attachment::{AttachmentKind, AttachmentRef};
pub use event::{
    now_ms, Event, EventId, ExecutionEvidence, FileChange, FileSnapshot, ProcessOutcome, Timestamp,
    ToolCall, Usage, WorkspaceSnapshot,
};
pub use session::{load, LoadedSession, SessionLog};
