use std::time::Duration;

use futures_util::{StreamExt, TryStreamExt};
use reqwest::header::{ACCEPT, RETRY_AFTER};
use serde::Serialize;
use serde_json::{json, Map, Value};
use tokio_util::codec::{FramedRead, LinesCodec};
use tokio_util::io::StreamReader;

use crate::message::{ChatMessage, CompletionRequest, ToolSpec};
use crate::sse::SseParser;
use crate::{EventStream, Provider, ProviderError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingMode {
    Enabled,
    Disabled,
}

impl ThinkingMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "enabled" | "on" | "true" => Some(Self::Enabled),
            "disabled" | "off" | "false" => Some(Self::Disabled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningEffort {
    None,
    Low,
    High,
    Max,
}

impl ReasoningEffort {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Low => "low",
            Self::High => "high",
            Self::Max => "max",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" => Some(Self::None),
            "minimal" | "low" => Some(Self::Low),
            "medium" | "high" | "xhigh" => Some(Self::High),
            "max" | "ultra" => Some(Self::Max),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    DeepSeek,
    OpenAi,
}

impl Preset {
    pub const ALL: [Self; 2] = [Self::DeepSeek, Self::OpenAi];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeepSeek => "deepseek",
            Self::OpenAi => "openai",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "deepseek" => Some(Self::DeepSeek),
            "openai" => Some(Self::OpenAi),
            _ => None,
        }
    }

    pub fn api_key_env(self) -> &'static str {
        match self {
            Self::DeepSeek => "DEEPSEEK_API_KEY",
            Self::OpenAi => "OPENAI_API_KEY",
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::DeepSeek => "https://api.deepseek.com",
            Self::OpenAi => "https://api.openai.com/v1",
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            Self::DeepSeek => "deepseek-flash",
            Self::OpenAi => "gpt-4o-mini",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    /// Whether the endpoint/model accepts OpenAI-compatible `image_url`
    /// content parts. Presets derive this from the catalog; custom endpoints
    /// opt in with [`OpenAiConfig::with_image_support`].
    pub supports_images: bool,
    /// DeepSeek-specific `thinking` control; `None` for plain OpenAI-compatible
    /// providers.
    pub thinking: Option<ThinkingMode>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub connect_timeout: Duration,
}

impl OpenAiConfig {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: api_key.into(),
            model: model.into(),
            supports_images: false,
            thinking: None,
            reasoning_effort: None,
            connect_timeout: Duration::from_secs(10),
        }
    }

    pub fn with_image_support(mut self, supports: bool) -> Self {
        self.supports_images = supports;
        self
    }

    pub fn with_thinking(mut self, mode: ThinkingMode) -> Self {
        self.thinking = Some(mode);
        self
    }

    pub fn with_reasoning_effort(mut self, effort: ReasoningEffort) -> Self {
        self.reasoning_effort = Some(effort);
        self
    }

    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    pub fn from_preset(preset: Preset) -> Result<Self, ProviderError> {
        let mut config = Self::preset_config(preset)?;
        config.api_key = required_env(preset.api_key_env())?;
        Ok(config)
    }

    /// Preset settings without a credential. Callers using `niucode-auth` set
    /// `api_key` after resolving env → keyring → file.
    pub fn preset_config(preset: Preset) -> Result<Self, ProviderError> {
        match preset {
            Preset::DeepSeek => {
                let base_url = std::env::var("DEEPSEEK_BASE_URL")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| preset.default_base_url().into());
                let model = std::env::var("DEEPSEEK_MODEL")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| preset.default_model().into());
                let thinking = match std::env::var("DEEPSEEK_THINKING") {
                    Ok(value) => Some(ThinkingMode::parse(&value).ok_or_else(|| {
                        ProviderError::Config(format!(
                            "DEEPSEEK_THINKING must be enabled or disabled, got {value:?}"
                        ))
                    })?),
                    Err(_) => Some(ThinkingMode::Enabled),
                };
                let reasoning_effort = match std::env::var("DEEPSEEK_REASONING_EFFORT") {
                    Ok(value) => Some(ReasoningEffort::parse(&value).ok_or_else(|| {
                        ProviderError::Config(format!(
                            "DEEPSEEK_REASONING_EFFORT must be none/low/high/max, got {value:?}"
                        ))
                    })?),
                    Err(_) => None,
                };

                Ok(Self {
                    base_url,
                    api_key: String::new(),
                    supports_images: crate::catalog::model_supports_images(&model),
                    model,
                    thinking,
                    reasoning_effort,
                    connect_timeout: Duration::from_secs(10),
                })
            }
            Preset::OpenAi => {
                let base_url = std::env::var("OPENAI_BASE_URL")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| preset.default_base_url().into());
                let model = std::env::var("OPENAI_MODEL")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| preset.default_model().into());

                Ok(Self {
                    base_url,
                    api_key: String::new(),
                    supports_images: crate::catalog::model_supports_images(&model),
                    model,
                    thinking: None,
                    reasoning_effort: None,
                    connect_timeout: Duration::from_secs(10),
                })
            }
        }
    }
}

fn required_env(name: &'static str) -> Result<String, ProviderError> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or(ProviderError::MissingApiKey {
            provider: if name.contains("DEEPSEEK") {
                "deepseek"
            } else {
                "openai"
            },
            env: name,
        })
}

pub struct OpenAiProvider {
    client: reqwest::Client,
    config: OpenAiConfig,
    name: String,
}

impl OpenAiProvider {
    fn ensure_image_support(&self, request: &CompletionRequest) -> Result<(), ProviderError> {
        if request.messages.iter().any(ChatMessage::has_images) && !self.config.supports_images {
            return Err(ProviderError::UnsupportedImages {
                model: self.config.model.clone(),
            });
        }
        Ok(())
    }

    pub fn new(config: OpenAiConfig) -> Result<Self, ProviderError> {
        Self::with_name("openai-compatible", config)
    }

    pub fn from_preset(preset: Preset) -> Result<Self, ProviderError> {
        Self::with_name(preset.as_str(), OpenAiConfig::from_preset(preset)?)
    }

    /// Build a provider from a preset config whose credential was resolved by
    /// `niucode-auth`. Keeps the preset name for diagnostics.
    pub fn for_preset(preset: Preset, config: OpenAiConfig) -> Result<Self, ProviderError> {
        Self::with_name(preset.as_str(), config)
    }

    fn with_name(name: impl Into<String>, config: OpenAiConfig) -> Result<Self, ProviderError> {
        if config.api_key.trim().is_empty() {
            return Err(ProviderError::Config("provider API key is empty".into()));
        }

        let client = reqwest::Client::builder()
            .connect_timeout(config.connect_timeout)
            .build()
            .map_err(|error| ProviderError::Network(error.to_string()))?;

        Ok(Self {
            client,
            config,
            name: name.into(),
        })
    }

    pub fn model(&self) -> &str {
        &self.config.model
    }

    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    fn endpoint(&self) -> String {
        let base = self.config.base_url.trim_end_matches('/');
        if base.ends_with("/chat/completions") {
            base.to_string()
        } else {
            format!("{base}/chat/completions")
        }
    }

    fn build_body(&self, request: &CompletionRequest) -> Value {
        let mut extra = Map::new();
        if let Some(thinking) = self.config.thinking {
            extra.insert("thinking".into(), json!({ "type": thinking.as_str() }));
        }

        let wire = WireRequest {
            model: &request.model,
            messages: &request.messages,
            tools: if request.tools.is_empty() {
                None
            } else {
                Some(request.tools.as_slice())
            },
            stream: true,
            stream_options: StreamOptions {
                include_usage: true,
            },
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            reasoning_effort: self.config.reasoning_effort.map(ReasoningEffort::as_str),
            extra,
        };

        serde_json::to_value(wire).expect("wire request is serializable")
    }
}

#[async_trait::async_trait]
impl Provider for OpenAiProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn stream(&self, request: CompletionRequest) -> Result<EventStream, ProviderError> {
        self.ensure_image_support(&request)?;
        let body = self.build_body(&request);

        let response = self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.config.api_key)
            .header(ACCEPT, "text/event-stream")
            .json(&body)
            .send()
            .await
            .map_err(|error| ProviderError::Network(error.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .map(Duration::from_secs);
            let body = response.text().await.unwrap_or_default();
            return Err(classify_http(status.as_u16(), &body, retry_after));
        }

        let parser = SseParser::default();
        let stream = async_stream::try_stream! {
            let mut parser = parser;
            let bytes = response
                .bytes_stream()
                .map_err(std::io::Error::other);
            let reader = StreamReader::new(bytes);
            let mut lines = FramedRead::new(
                reader,
                LinesCodec::new_with_max_length(1_048_576),
            );

            while let Some(line) = lines.next().await {
                let line = line.map_err(|error| ProviderError::Decode(error.to_string()))?;
                for event in parser.push_line(&line)? {
                    yield event;
                }
            }

            for event in parser.finish()? {
                yield event;
            }
        };

        Ok(Box::pin(stream))
    }
}

#[derive(Serialize)]
struct WireRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [ToolSpec]>,
    stream: bool,
    stream_options: StreamOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'static str>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}

fn classify_http(status: u16, body: &str, retry_after: Option<Duration>) -> ProviderError {
    let lower = body.to_ascii_lowercase();
    let message = truncate(body, 600);

    if matches!(status, 401 | 403) {
        return ProviderError::Auth(message);
    }

    if matches!(status, 400 | 422)
        && (lower.contains("context length")
            || lower.contains("maximum context")
            || lower.contains("context window")
            || lower.contains("too long"))
    {
        return ProviderError::ContextOverflow(message);
    }

    let retryable = matches!(status, 408 | 425 | 429 | 500 | 502 | 503 | 504);
    ProviderError::Http {
        status,
        message,
        retryable,
        retry_after,
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_util::StreamExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::*;
    use crate::message::ToolSpec;
    use crate::StreamEvent;
    use niucode_core::Usage;

    #[test]
    fn image_parts_serialize_and_text_only_models_are_rejected() {
        let request = CompletionRequest::new("model").with_message(ChatMessage::user_with_images(
            "look",
            vec!["data:image/png;base64,AAAA".into()],
        ));
        assert!(request.messages[0].has_images());
        assert_eq!(
            request.messages[0].content.as_ref().unwrap().image_count(),
            1
        );

        assert!(crate::catalog::model_supports_images("gpt-4o"));
        assert!(!crate::catalog::model_supports_images("deepseek-flash"));
        let text_only = OpenAiProvider::new(OpenAiConfig::new(
            "https://example.invalid",
            "key",
            "deepseek-flash",
        ))
        .unwrap();
        assert!(matches!(
            text_only.ensure_image_support(&request),
            Err(ProviderError::UnsupportedImages { .. })
        ));

        let vision = OpenAiProvider::new(
            OpenAiConfig::new("https://example.invalid", "key", "gpt-4o").with_image_support(true),
        )
        .unwrap();
        assert!(vision.ensure_image_support(&request).is_ok());
        let body = vision.build_body(&request);
        assert_eq!(body["messages"][0]["content"][0]["type"], "text");
        assert_eq!(body["messages"][0]["content"][0]["text"], "look");
        assert_eq!(body["messages"][0]["content"][1]["type"], "image_url");
        assert_eq!(
            body["messages"][0]["content"][1]["image_url"]["url"],
            "data:image/png;base64,AAAA"
        );
    }

    #[test]
    fn deepseek_body_contains_thinking_and_stream_options() {
        let config = OpenAiConfig::new("https://api.deepseek.com", "key", "deepseek-flash")
            .with_thinking(ThinkingMode::Enabled)
            .with_reasoning_effort(ReasoningEffort::High);
        let provider = OpenAiProvider::new(config).expect("provider");
        let request = CompletionRequest::new("deepseek-flash")
            .with_message(ChatMessage::system("be useful"))
            .with_message(ChatMessage::user("hi"))
            .with_tool(ToolSpec::function(
                "read",
                "read a file",
                json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            ));

        let body = provider.build_body(&request);
        assert_eq!(body["stream"], true);
        assert_eq!(body["stream_options"]["include_usage"], true);
        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["reasoning_effort"], "high");
        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["function"]["name"], "read");
    }

    #[test]
    fn assistant_message_keeps_reasoning_for_tool_rounds() {
        let message = ChatMessage::assistant(
            Some("done".into()),
            Some("think".into()),
            vec![niucode_core::ToolCall {
                id: "call_1".into(),
                name: "read".into(),
                input: json!({"path": "src/lib.rs"}),
            }],
        );
        let value = serde_json::to_value(message).expect("serialize");
        assert_eq!(value["reasoning_content"], "think");
        assert_eq!(value["tool_calls"][0]["function"]["name"], "read");
        assert_eq!(
            value["tool_calls"][0]["function"]["arguments"],
            "{\"path\":\"src/lib.rs\"}"
        );
    }

    #[test]
    fn classifies_context_overflow() {
        let error = classify_http(
            400,
            "This model's maximum context length is 131072 tokens",
            None,
        );
        assert!(error.is_context_overflow());
    }

    #[tokio::test]
    async fn streams_from_openai_compatible_sse_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("local addr");

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut request = [0_u8; 4096];
            let _ = socket.read(&mut request).await.expect("read request");

            let body = concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"},\"finish_reason\":null}]}\n\n",
                "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":1,\"prompt_tokens_details\":{\"cached_tokens\":2}}}\n\n",
                "data: [DONE]\n\n"
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write response");
        });

        let base_url = format!("http://{address}");
        let provider = OpenAiProvider::new(OpenAiConfig::new(base_url, "test-key", "test-model"))
            .expect("provider");
        let request = CompletionRequest::new("test-model").with_message(ChatMessage::user("hello"));

        let mut stream = provider.stream(request).await.expect("stream");
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event.expect("event"));
        }

        assert!(events.contains(&StreamEvent::TextDelta("hello".into())));
        assert!(events.contains(&StreamEvent::Usage(Usage {
            input_tokens: 3,
            output_tokens: 1,
            cached_tokens: 2,
            reasoning_tokens: 0,
        })));
        assert!(matches!(
            events.last(),
            Some(StreamEvent::Done { finish_reason: Some(reason) }) if reason == "stop"
        ));

        server.await.expect("server task");
    }

    #[tokio::test]
    async fn surfaces_retry_after_on_429() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("local addr");

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut request = [0_u8; 4096];
            let _ = socket.read(&mut request).await.expect("read request");
            let body = "rate limited";
            let response = format!(
                "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 2\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write response");
        });

        let base_url = format!("http://{address}");
        let provider = OpenAiProvider::new(OpenAiConfig::new(base_url, "test-key", "test-model"))
            .expect("provider");
        let error = match provider
            .stream(CompletionRequest::new("test-model").with_message(ChatMessage::user("hi")))
            .await
        {
            Ok(_) => panic!("429 must fail"),
            Err(error) => error,
        };

        assert!(error.is_retryable());
        assert_eq!(error.retry_after(), Some(Duration::from_secs(2)));

        server.await.expect("server task");
    }
}
