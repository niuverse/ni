use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures_util::{stream, StreamExt};
use niucode_core::Usage;
use serde::{Deserialize, Serialize};

use crate::{CompletionRequest, EventStream, Provider, ProviderError, StreamEvent, ToolCallDelta};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecordedEvent {
    TextDelta {
        text: String,
    },
    ReasoningDelta {
        text: String,
    },
    ToolCallDelta {
        index: u32,
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        arguments: String,
    },
    Usage {
        usage: Usage,
    },
    Done {
        #[serde(default)]
        finish_reason: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedFixture {
    pub schema_version: u32,
    pub turns: Vec<Vec<RecordedEvent>>,
}

impl From<&StreamEvent> for RecordedEvent {
    fn from(event: &StreamEvent) -> Self {
        match event {
            StreamEvent::TextDelta(text) => Self::TextDelta { text: text.clone() },
            StreamEvent::ReasoningDelta(text) => Self::ReasoningDelta { text: text.clone() },
            StreamEvent::ToolCallDelta(call) => Self::ToolCallDelta {
                index: call.index,
                id: call.id.clone(),
                name: call.name.clone(),
                arguments: call.arguments.clone(),
            },
            StreamEvent::Usage(usage) => Self::Usage {
                usage: usage.clone(),
            },
            StreamEvent::Done { finish_reason } => Self::Done {
                finish_reason: finish_reason.clone(),
            },
        }
    }
}

impl From<RecordedEvent> for StreamEvent {
    fn from(event: RecordedEvent) -> Self {
        match event {
            RecordedEvent::TextDelta { text } => Self::TextDelta(text),
            RecordedEvent::ReasoningDelta { text } => Self::ReasoningDelta(text),
            RecordedEvent::ToolCallDelta {
                index,
                id,
                name,
                arguments,
            } => Self::ToolCallDelta(ToolCallDelta {
                index,
                id,
                name,
                arguments,
            }),
            RecordedEvent::Usage { usage } => Self::Usage(usage),
            RecordedEvent::Done { finish_reason } => Self::Done { finish_reason },
        }
    }
}

/// Replay normalized provider events from a recorded fixture, with no network.
pub struct ReplayProvider {
    name: String,
    turns: Mutex<VecDeque<Vec<StreamEvent>>>,
    requests: Mutex<Vec<CompletionRequest>>,
}

impl ReplayProvider {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProviderError> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path).map_err(|error| {
            ProviderError::Config(format!("read fixture {}: {error}", path.display()))
        })?;
        let fixture: RecordedFixture = serde_json::from_str(&raw).map_err(|error| {
            ProviderError::Config(format!("parse fixture {}: {error}", path.display()))
        })?;
        Self::from_fixture(fixture)
    }

    pub fn from_fixture(fixture: RecordedFixture) -> Result<Self, ProviderError> {
        if fixture.schema_version != 1 {
            return Err(ProviderError::Config(format!(
                "unsupported fixture schema {}",
                fixture.schema_version
            )));
        }

        let turns = fixture
            .turns
            .into_iter()
            .map(|turn| turn.into_iter().map(StreamEvent::from).collect())
            .collect();

        Ok(Self {
            name: "replay".into(),
            turns: Mutex::new(turns),
            requests: Mutex::new(Vec::new()),
        })
    }

    pub fn requests(&self) -> Vec<CompletionRequest> {
        self.requests.lock().expect("replay request lock").clone()
    }
}

#[async_trait::async_trait]
impl Provider for ReplayProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn stream(&self, request: CompletionRequest) -> Result<EventStream, ProviderError> {
        self.requests
            .lock()
            .expect("replay request lock")
            .push(request);

        let turn = self
            .turns
            .lock()
            .expect("replay turn lock")
            .pop_front()
            .ok_or_else(|| ProviderError::Config("replay fixture exhausted".into()))?;

        Ok(Box::pin(stream::iter(turn.into_iter().map(Ok))))
    }
}

/// Wrap a provider and persist each normalized turn as a replay fixture.
pub struct RecordingProvider {
    inner: Arc<dyn Provider>,
    path: PathBuf,
    name: String,
    turns: Arc<Mutex<Vec<Vec<RecordedEvent>>>>,
}

impl RecordingProvider {
    pub fn new(inner: Arc<dyn Provider>, path: impl Into<PathBuf>) -> Self {
        let name = format!("{} (recording)", inner.name());
        Self {
            inner,
            path: path.into(),
            name,
            turns: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[async_trait::async_trait]
impl Provider for RecordingProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn stream(&self, request: CompletionRequest) -> Result<EventStream, ProviderError> {
        let mut inner = self.inner.stream(request).await?;
        let recorded = Arc::new(Mutex::new(Vec::<RecordedEvent>::new()));
        let turns = self.turns.clone();
        let path = self.path.clone();
        let turn_events = recorded.clone();

        let stream = async_stream::try_stream! {
            while let Some(event) = inner.next().await {
                let event = event?;
                turn_events
                    .lock()
                    .expect("recording event lock")
                    .push(RecordedEvent::from(&event));
                yield event;
            }

            let turn = turn_events
                .lock()
                .expect("recording event lock")
                .drain(..)
                .collect::<Vec<_>>();
            let snapshot = {
                let mut turns = turns.lock().expect("recording turns lock");
                turns.push(turn);
                turns.clone()
            };
            write_fixture(&path, snapshot)?;
        };

        Ok(Box::pin(stream))
    }
}

fn write_fixture(path: &Path, turns: Vec<Vec<RecordedEvent>>) -> Result<(), ProviderError> {
    let fixture = RecordedFixture {
        schema_version: 1,
        turns,
    };
    let data = serde_json::to_string_pretty(&fixture)
        .map_err(|error| ProviderError::Config(format!("serialize fixture: {error}")))?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| ProviderError::Config(format!("create fixture dir: {error}")))?;
    }
    fs::write(path, data).map_err(|error| {
        ProviderError::Config(format!("write fixture {}: {error}", path.display()))
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::ScriptedProvider;

    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("niucode-fixture-{tag}-{stamp}-{sequence}.json"))
    }

    async fn collect(mut stream: EventStream) -> Vec<StreamEvent> {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event.expect("stream event"));
        }
        events
    }

    #[tokio::test]
    async fn record_then_replay_round_trip() {
        let path = temp_path("roundtrip");
        let inner = Arc::new(ScriptedProvider::from_turns(vec![vec![
            StreamEvent::TextDelta("hello".into()),
            StreamEvent::Usage(Usage {
                input_tokens: 3,
                output_tokens: 1,
                cached_tokens: 2,
                reasoning_tokens: 0,
            }),
            StreamEvent::Done {
                finish_reason: Some("stop".into()),
            },
        ]]));
        let recorder = RecordingProvider::new(inner, &path);

        let request = CompletionRequest::new("test");
        let events = collect(recorder.stream(request).await.expect("stream")).await;
        assert_eq!(events.len(), 3);
        assert!(path.exists());

        let replay = ReplayProvider::load(&path).expect("load fixture");
        let replayed = collect(
            replay
                .stream(CompletionRequest::new("test"))
                .await
                .expect("replay"),
        )
        .await;
        assert_eq!(replayed, events);

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn exhausted_replay_is_an_error() {
        let path = temp_path("exhausted");
        let fixture = RecordedFixture {
            schema_version: 1,
            turns: vec![vec![RecordedEvent::Done {
                finish_reason: Some("stop".into()),
            }]],
        };
        fs::write(&path, serde_json::to_string(&fixture).expect("fixture")).expect("write");
        let replay = ReplayProvider::load(&path).expect("load");

        collect(
            replay
                .stream(CompletionRequest::new("test"))
                .await
                .expect("first"),
        )
        .await;
        let error = match replay.stream(CompletionRequest::new("test")).await {
            Ok(_) => panic!("second stream must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("exhausted"));

        let _ = fs::remove_file(path);
    }
}
