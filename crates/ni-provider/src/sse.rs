use ni_core::Usage;
use serde::Deserialize;

use crate::{ProviderError, StreamEvent, ToolCallDelta};

/// Incremental SSE decoder for OpenAI-compatible chat completion streams.
///
/// It accepts text chunks of arbitrary size, buffers partial lines, joins
/// multi-line `data:` fields, ignores comments/keepalives, and turns each JSON
/// payload into normalized [`StreamEvent`]s. `[DONE]` emits the final event.
#[derive(Debug, Default)]
pub struct SseParser {
    buffer: String,
    data: Vec<String>,
    finish_reason: Option<String>,
    done: bool,
}

impl SseParser {
    pub fn push_str(&mut self, chunk: &str) -> Result<Vec<StreamEvent>, ProviderError> {
        let mut events = Vec::new();
        self.buffer.push_str(chunk);

        while let Some(position) = self.buffer.find('\n') {
            let line = self.buffer[..position].trim_end_matches('\r').to_string();
            self.buffer.drain(..=position);
            self.handle_line(&line, &mut events)?;
        }

        Ok(events)
    }

    /// Feed one complete line (without its trailing newline). Used by the
    /// streaming HTTP path, which decodes lines before handing them over.
    pub fn push_line(&mut self, line: &str) -> Result<Vec<StreamEvent>, ProviderError> {
        let mut events = Vec::new();
        self.handle_line(line.trim_end_matches('\r'), &mut events)?;
        Ok(events)
    }

    /// Flush a trailing line and ensure a `Done` event is emitted exactly once.
    pub fn finish(&mut self) -> Result<Vec<StreamEvent>, ProviderError> {
        let mut events = Vec::new();

        if !self.buffer.is_empty() {
            let line = std::mem::take(&mut self.buffer);
            self.handle_line(&line, &mut events)?;
        }

        self.dispatch(&mut events)?;

        if !self.done {
            self.done = true;
            events.push(StreamEvent::Done {
                finish_reason: self.finish_reason.clone(),
            });
        }

        Ok(events)
    }

    fn handle_line(
        &mut self,
        line: &str,
        events: &mut Vec<StreamEvent>,
    ) -> Result<(), ProviderError> {
        if line.is_empty() {
            self.dispatch(events)?;
            return Ok(());
        }

        if line.starts_with(':') {
            return Ok(());
        }

        if let Some(rest) = line.strip_prefix("data:") {
            let payload = rest.strip_prefix(' ').unwrap_or(rest);
            if !payload.is_empty() {
                self.data.push(payload.to_string());
            }
        }

        // `event:`, `id:`, and `retry:` do not change the normalized payload.
        Ok(())
    }

    fn dispatch(&mut self, events: &mut Vec<StreamEvent>) -> Result<(), ProviderError> {
        if self.data.is_empty() {
            return Ok(());
        }

        let payload = self.data.join("\n");
        self.data.clear();

        if payload.trim() == "[DONE]" {
            if !self.done {
                self.done = true;
                events.push(StreamEvent::Done {
                    finish_reason: self.finish_reason.clone(),
                });
            }
            return Ok(());
        }

        let chunk: WireChunk = serde_json::from_str(&payload).map_err(|error| {
            ProviderError::Decode(format!("{error}: {}", truncate(&payload, 240)))
        })?;

        if let Some(error) = chunk.error {
            return Err(ProviderError::Decode(format!(
                "provider error: {}",
                truncate(&error.message, 240)
            )));
        }

        if let Some(usage) = chunk.usage {
            events.push(StreamEvent::Usage(usage.into_usage()));
        }

        for choice in chunk.choices {
            if let Some(reasoning) = choice.delta.reasoning_content {
                if !reasoning.is_empty() {
                    events.push(StreamEvent::ReasoningDelta(reasoning));
                }
            }

            if let Some(content) = choice.delta.content {
                if !content.is_empty() {
                    events.push(StreamEvent::TextDelta(content));
                }
            }

            if let Some(tool_calls) = choice.delta.tool_calls {
                for (fallback, tool_call) in tool_calls.into_iter().enumerate() {
                    let function = tool_call.function.unwrap_or_default();
                    events.push(StreamEvent::ToolCallDelta(ToolCallDelta {
                        index: tool_call.index.unwrap_or(fallback as u32),
                        id: tool_call.id,
                        name: function.name,
                        arguments: function.arguments.unwrap_or_default(),
                    }));
                }
            }

            if let Some(reason) = choice.finish_reason {
                self.finish_reason = Some(reason);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct WireChunk {
    #[serde(default)]
    choices: Vec<WireChoice>,
    #[serde(default)]
    usage: Option<WireUsage>,
    #[serde(default)]
    error: Option<WireError>,
}

#[derive(Debug, Deserialize)]
struct WireChoice {
    #[serde(default)]
    delta: WireDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct WireDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<WireToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct WireToolCallDelta {
    #[serde(default)]
    index: Option<u32>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<WireFunctionDelta>,
}

#[derive(Debug, Default, Deserialize)]
struct WireFunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct WireUsage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
    #[serde(default)]
    prompt_tokens_details: Option<PromptTokensDetails>,
    #[serde(default)]
    completion_tokens_details: Option<CompletionTokensDetails>,
    #[serde(default)]
    prompt_cache_hit_tokens: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct PromptTokensDetails {
    #[serde(default)]
    cached_tokens: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct CompletionTokensDetails {
    #[serde(default)]
    reasoning_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct WireError {
    message: String,
}

impl WireUsage {
    fn into_usage(self) -> Usage {
        let cached_tokens = self
            .prompt_tokens_details
            .as_ref()
            .and_then(|details| details.cached_tokens)
            .or(self.prompt_cache_hit_tokens)
            .unwrap_or(0);

        let reasoning_tokens = self
            .completion_tokens_details
            .as_ref()
            .and_then(|details| details.reasoning_tokens)
            .unwrap_or(0);

        Usage {
            input_tokens: self.prompt_tokens,
            output_tokens: self.completion_tokens,
            cached_tokens,
            reasoning_tokens,
        }
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
    use serde_json::json;

    use super::*;

    fn feed(parser: &mut SseParser, text: &str) -> Vec<StreamEvent> {
        let mut events = Vec::new();
        for chunk in text.as_bytes().chunks(7) {
            let chunk = std::str::from_utf8(chunk).expect("test fixture is ASCII");
            events.extend(parser.push_str(chunk).expect("parse chunk"));
        }
        events.extend(parser.finish().expect("finish parser"));
        events
    }

    #[test]
    fn parses_text_reasoning_tool_calls_usage_and_done() {
        let chunks = [
            json!({
                "choices": [{
                    "delta": {"reasoning_content": "think"},
                    "finish_reason": null
                }]
            }),
            json!({
                "choices": [{
                    "delta": {"content": "Hel"},
                    "finish_reason": null
                }]
            }),
            json!({
                "choices": [{
                    "delta": {"content": "lo"},
                    "finish_reason": null
                }]
            }),
            json!({
                "choices": [{
                    "delta": {"tool_calls": [{
                        "index": 0,
                        "id": "call_1",
                        "function": {"name": "read", "arguments": "{\"path\":"}
                    }]},
                    "finish_reason": null
                }]
            }),
            json!({
                "choices": [{
                    "delta": {"tool_calls": [{
                        "index": 0,
                        "function": {"arguments": "\"src/lib.rs\"}"}
                    }]},
                    "finish_reason": "tool_calls"
                }]
            }),
            json!({
                "choices": [{
                    "delta": {},
                    "finish_reason": "tool_calls"
                }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 5,
                    "prompt_tokens_details": {"cached_tokens": 4},
                    "completion_tokens_details": {"reasoning_tokens": 2}
                }
            }),
        ];

        let mut text = String::new();
        for chunk in chunks {
            text.push_str(&format!("data: {chunk}\n\n"));
        }
        text.push_str("data: [DONE]\n\n");

        let events = feed(&mut SseParser::default(), &text);

        assert!(events.contains(&StreamEvent::ReasoningDelta("think".into())));
        assert!(events.contains(&StreamEvent::TextDelta("Hel".into())));
        assert!(events.contains(&StreamEvent::TextDelta("lo".into())));
        assert!(events.contains(&StreamEvent::ToolCallDelta(ToolCallDelta {
            index: 0,
            id: Some("call_1".into()),
            name: Some("read".into()),
            arguments: "{\"path\":".into(),
        })));
        assert!(events.contains(&StreamEvent::ToolCallDelta(ToolCallDelta {
            index: 0,
            id: None,
            name: None,
            arguments: "\"src/lib.rs\"}".into(),
        })));
        assert!(events.contains(&StreamEvent::Usage(Usage {
            input_tokens: 10,
            output_tokens: 5,
            cached_tokens: 4,
            reasoning_tokens: 2,
        })));
        assert_eq!(
            events.last(),
            Some(&StreamEvent::Done {
                finish_reason: Some("tool_calls".into())
            })
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, StreamEvent::Done { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn parses_recorded_deepseek_fixture() {
        let fixture = include_str!("../tests/fixtures/deepseek-tool-call.sse");
        let events = feed(&mut SseParser::default(), fixture);

        assert!(events.contains(&StreamEvent::ReasoningDelta("need a file".into())));
        assert!(events.contains(&StreamEvent::TextDelta("I will read it.".into())));
        assert!(events.contains(&StreamEvent::ToolCallDelta(ToolCallDelta {
            index: 0,
            id: Some("call_1".into()),
            name: Some("read".into()),
            arguments: "{\"path\":".into(),
        })));
        assert!(events.contains(&StreamEvent::Usage(Usage {
            input_tokens: 42,
            output_tokens: 12,
            cached_tokens: 30,
            reasoning_tokens: 4,
        })));
        assert!(matches!(
            events.last(),
            Some(StreamEvent::Done {
                finish_reason: Some(reason)
            }) if reason == "tool_calls"
        ));
    }

    #[test]
    fn emits_done_without_done_sentinel() {
        let text = format!(
            "data: {}\n\n",
            json!({
                "choices": [{"delta": {"content": "ok"}, "finish_reason": "stop"}]
            })
        );

        let events = feed(&mut SseParser::default(), &text);
        assert!(events.contains(&StreamEvent::TextDelta("ok".into())));
        assert_eq!(
            events.last(),
            Some(&StreamEvent::Done {
                finish_reason: Some("stop".into())
            })
        );
    }

    #[test]
    fn surfaces_invalid_json_as_decode_error() {
        let mut parser = SseParser::default();
        let error = parser
            .push_str("data: {not json}\n\n")
            .expect_err("invalid JSON must fail");
        assert!(matches!(error, ProviderError::Decode(_)));
    }
}
