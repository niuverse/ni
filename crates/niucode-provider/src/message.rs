use niucode_core::ToolCall;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl MessageContent {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Parts(_) => None,
        }
    }
    pub fn has_images(&self) -> bool {
        match self {
            Self::Text(_) => false,
            Self::Parts(parts) => parts
                .iter()
                .any(|part| matches!(part, ContentPart::ImageUrl { .. })),
        }
    }

    pub fn image_count(&self) -> usize {
        match self {
            Self::Text(_) => 0,
            Self::Parts(parts) => parts
                .iter()
                .filter(|part| matches!(part, ContentPart::ImageUrl { .. }))
                .count(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,
    /// DeepSeek extension. Must be echoed back when the request carries tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<WireToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: Some(MessageContent::text(text)),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Some(MessageContent::text(text)),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// User text plus one or more OpenAI-compatible `image_url` parts. The
    /// caller owns the data URL or remote URL; this crate does not fetch or
    /// persist images.
    pub fn user_with_images(text: impl Into<String>, images: Vec<String>) -> Self {
        let mut parts = vec![ContentPart::Text { text: text.into() }];
        parts.extend(images.into_iter().map(|url| ContentPart::ImageUrl {
            image_url: ImageUrl { url, detail: None },
        }));
        Self {
            role: Role::User,
            content: Some(MessageContent::Parts(parts)),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn text(&self) -> Option<&str> {
        self.content.as_ref().and_then(MessageContent::as_text)
    }

    pub fn has_images(&self) -> bool {
        self.content
            .as_ref()
            .is_some_and(MessageContent::has_images)
    }

    pub fn assistant(
        text: Option<String>,
        reasoning: Option<String>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        let tool_calls = if tool_calls.is_empty() {
            None
        } else {
            Some(
                tool_calls
                    .iter()
                    .map(WireToolCall::from_tool_call)
                    .collect(),
            )
        };

        Self {
            role: Role::Assistant,
            content: text
                .filter(|value| !value.is_empty())
                .map(MessageContent::text),
            reasoning_content: reasoning.filter(|value| !value.is_empty()),
            tool_calls,
            tool_call_id: None,
        }
    }

    pub fn tool(call_id: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: Some(MessageContent::text(output)),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: Some(call_id.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: WireFunction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireFunction {
    pub name: String,
    pub arguments: String,
}

impl WireToolCall {
    pub fn from_tool_call(call: &ToolCall) -> Self {
        let arguments = if call.input.is_null() {
            "{}".to_string()
        } else {
            serde_json::to_string(&call.input).unwrap_or_else(|_| "{}".to_string())
        };

        Self {
            id: call.id.clone(),
            kind: "function".into(),
            function: WireFunction {
                name: call.name.clone(),
                arguments,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolSpec {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: FunctionSpec,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FunctionSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl ToolSpec {
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
    ) -> Self {
        Self {
            kind: "function".into(),
            function: FunctionSpec {
                name: name.into(),
                description: description.into(),
                parameters,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolSpec>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

impl CompletionRequest {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            messages: Vec::new(),
            tools: Vec::new(),
            temperature: None,
            max_tokens: None,
        }
    }

    pub fn with_message(mut self, message: ChatMessage) -> Self {
        self.messages.push(message);
        self
    }

    pub fn with_tool(mut self, tool: ToolSpec) -> Self {
        self.tools.push(tool);
        self
    }
}
