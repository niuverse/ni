use std::collections::VecDeque;
use std::sync::Mutex;

use futures_util::stream;

use crate::{CompletionRequest, EventStream, Provider, ProviderError, StreamEvent};

/// Deterministic provider for tests and offline loop development.
///
/// Each `stream` call consumes one scripted turn. A turn is a sequence of
/// normalized events, typically ending with `Done`.
pub struct ScriptedProvider {
    name: String,
    turns: Mutex<VecDeque<Vec<Result<StreamEvent, ProviderError>>>>,
    requests: Mutex<Vec<CompletionRequest>>,
}

impl ScriptedProvider {
    pub fn new(events: Vec<StreamEvent>) -> Self {
        Self::from_turns(vec![events])
    }

    pub fn from_turns(turns: Vec<Vec<StreamEvent>>) -> Self {
        let turns = turns
            .into_iter()
            .map(|turn| turn.into_iter().map(Ok).collect())
            .collect();
        Self::from_results(turns)
    }

    pub fn with_results(events: Vec<Result<StreamEvent, ProviderError>>) -> Self {
        Self::from_results(vec![events])
    }

    pub fn from_results(turns: Vec<Vec<Result<StreamEvent, ProviderError>>>) -> Self {
        Self {
            name: "scripted".into(),
            turns: Mutex::new(turns.into()),
            requests: Mutex::new(Vec::new()),
        }
    }

    pub fn requests(&self) -> Vec<CompletionRequest> {
        self.requests.lock().expect("scripted request lock").clone()
    }
}

#[async_trait::async_trait]
impl Provider for ScriptedProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn stream(&self, request: CompletionRequest) -> Result<EventStream, ProviderError> {
        self.requests
            .lock()
            .expect("scripted request lock")
            .push(request);

        let events = self
            .turns
            .lock()
            .expect("scripted turn lock")
            .pop_front()
            .unwrap_or_default();

        Ok(Box::pin(stream::iter(events)))
    }
}

#[cfg(test)]
mod tests {
    use futures_util::StreamExt;

    use super::*;
    use crate::ChatMessage;

    #[tokio::test]
    async fn replays_turns_and_records_requests() {
        let provider = ScriptedProvider::from_turns(vec![
            vec![
                StreamEvent::TextDelta("first".into()),
                StreamEvent::Done {
                    finish_reason: Some("stop".into()),
                },
            ],
            vec![
                StreamEvent::TextDelta("second".into()),
                StreamEvent::Done {
                    finish_reason: Some("stop".into()),
                },
            ],
        ]);

        for expected in ["first", "second"] {
            let request = CompletionRequest::new("test").with_message(ChatMessage::user("hi"));
            let mut events = provider.stream(request).await.expect("stream");
            let mut seen = Vec::new();
            while let Some(event) = events.next().await {
                seen.push(event.expect("event"));
            }
            assert_eq!(seen[0], StreamEvent::TextDelta(expected.into()));
        }

        assert_eq!(provider.requests().len(), 2);
        assert_eq!(provider.requests()[0].model, "test");
    }
}
