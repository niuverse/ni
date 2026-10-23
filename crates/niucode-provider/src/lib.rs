//! Provider boundary for niucode.
//!
//! The loop talks to a provider through [`Provider`] and consumes a stream of
//! normalized [`StreamEvent`]s. Wire formats stay in this crate; `niucode-core`
//! never sees SSE, HTTP, or provider-specific JSON.

mod catalog;
mod error;
mod fixture;
mod message;
mod openai;
mod scripted;
mod sse;

pub use catalog::{
    context_window, derived_context_budget, find_model, find_provider, providers, ModelDescriptor,
    ModelSelection, ProviderDescriptor, DEFAULT_CONTEXT_WINDOW,
};
pub use error::ProviderError;
pub use fixture::{RecordedEvent, RecordedFixture, RecordingProvider, ReplayProvider};
pub use message::{
    ChatMessage, CompletionRequest, ContentPart, FunctionSpec, ImageUrl, MessageContent, Role,
    ToolSpec, WireFunction, WireToolCall,
};
pub use openai::{OpenAiConfig, OpenAiProvider, Preset, ReasoningEffort, ThinkingMode};
pub use scripted::ScriptedProvider;
pub use sse::SseParser;

use std::pin::Pin;

use futures_util::Stream;
use niucode_core::Usage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallDelta {
    pub index: u32,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    TextDelta(String),
    ReasoningDelta(String),
    ToolCallDelta(ToolCallDelta),
    Usage(Usage),
    Done { finish_reason: Option<String> },
}

pub type EventStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, ProviderError>> + Send>>;

#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    async fn stream(&self, request: CompletionRequest) -> Result<EventStream, ProviderError>;
}
