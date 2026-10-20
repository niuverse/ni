use std::collections::VecDeque;
use std::sync::Mutex;

use futures_util::stream;

use crate::{CompletionRequest, EventStream, Provider, ProviderError, StreamEvent};

/// Deterministic provider for tests and offline loop development.
pub struct ScriptedProvider {
    name: String,
    events: Mutex<VecDeque<Result<StreamEvent, ProviderError>>>,
    requests: Mutex<Vec<CompletionRequest>>,
}

impl ScriptedProvider {
    pub fn new(events: Vec<StreamEvent>) -> Self {
        Self::with_results(events.into_iter().map(Ok).collect())
    }

    pub fn with_results(events: Vec<Result<StreamEvent, ProviderError>>) -> Self {
        Self {
            name: "scripted".into(),
            events: Mutex::new(events.into()),
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
            .events
            .lock()
            .expect("scripted event lock")
            .drain(..)
            .collect::<Vec<_>>();

        Ok(Box::pin(stream::iter(events)))
    }
}

#[cfg(test)]
mod tests {
    use futures_util::StreamExt;

    use super::*;
    use crate::ChatMessage;

    #[tokio::test]
    async fn replays_script_and_records_request() {
        let provider = ScriptedProvider::new(vec![
            StreamEvent::TextDelta("hello".into()),
            StreamEvent::Done {
                finish_reason: Some("stop".into()),
            },
        ]);
        let request = CompletionRequest::new("test").with_message(ChatMessage::user("hi"));

        let mut events = provider.stream(request).await.expect("stream");
        let mut seen = Vec::new();
        while let Some(event) = events.next().await {
            seen.push(event.expect("event"));
        }

        assert_eq!(seen.len(), 2);
        assert_eq!(provider.requests().len(), 1);
        assert_eq!(provider.requests()[0].model, "test");
    }
}
