//! Core primitives for niucode.
//!
//! The crate intentionally contains no provider, tool, or UI code. It owns the
//! event log and the projection rules that everything else builds on.

pub mod event;
pub mod session;

pub use event::{now_ms, Event, EventId, Timestamp, ToolCall, Usage};
pub use session::{load, LoadedSession, SessionLog};
