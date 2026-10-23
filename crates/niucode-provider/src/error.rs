use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("missing API key for {provider}: set {env}")]
    MissingApiKey {
        provider: &'static str,
        env: &'static str,
    },
    #[error("provider config error: {0}")]
    Config(String),
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("context overflow: {0}")]
    ContextOverflow(String),
    #[error("model {model} does not support image input; choose a vision model or enable image support explicitly")]
    UnsupportedImages { model: String },
    #[error("http {status}: {message}")]
    Http {
        status: u16,
        message: String,
        retryable: bool,
        retry_after: Option<Duration>,
    },
    #[error("network error: {0}")]
    Network(String),
    #[error("stream decode error: {0}")]
    Decode(String),
    #[error("provider cancelled")]
    Cancelled,
}

impl ProviderError {
    /// Whether the whole turn can be retried safely. Once a stream has started
    /// producing output, retrying in place is not safe; the loop should start a
    /// new turn instead.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Http { retryable, .. } => *retryable,
            Self::Network(_) => true,
            // A provider can return an error payload or a truncated SSE frame;
            // this is often a transient upstream/proxy failure. The agent only
            // retries when the stream has not emitted any event yet.
            Self::Decode(_) => true,
            _ => false,
        }
    }

    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Http { retry_after, .. } => *retry_after,
            _ => None,
        }
    }

    pub fn is_context_overflow(&self) -> bool {
        matches!(self, Self::ContextOverflow(_))
    }
}
