//! In-process control plane. The loop and control messages share one log writer.
use crate::{AgentConfig, AgentEvent, ApprovalRequest, RunOutcome};
use anyhow::Result;
use niucode_core::{load, now_ms, Event, SessionLog};
use niucode_provider::{ChatMessage, Provider};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use tokio::sync::{mpsc, watch};

const QUEUE_CAPACITY: usize = 32;
const MAX_MESSAGE_BYTES: usize = 65_536;

#[derive(Debug, Clone)]
pub enum RuntimeCommand {
    Submit {
        id: String,
        text: String,
        queued: bool,
    },
    Approve {
        request_id: String,
        allowed: bool,
    },
}
#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    Agent(AgentEvent),
    Input {
        id: String,
        queued: bool,
        applied: bool,
    },
    Approval {
        request_id: String,
        call_id: String,
        request: ApprovalRequest,
    },
    TaskDone {
        id: String,
        outcome: RunOutcome,
    },
    InputRejected {
        id: String,
        reason: String,
    },
    Notice(String),
}

pub struct RuntimeClient {
    pub commands: mpsc::Sender<RuntimeCommand>,
    pub events: mpsc::Receiver<RuntimeEvent>,
    cancel: watch::Sender<bool>,
}
impl RuntimeClient {
    pub fn cancel(&self) {
        let _ = self.cancel.send(true);
    }
}
pub struct RuntimeEndpoint {
    commands: mpsc::Receiver<RuntimeCommand>,
    events: mpsc::Sender<RuntimeEvent>,
    cancel: watch::Receiver<bool>,
}
pub fn runtime_channel() -> (RuntimeClient, RuntimeEndpoint) {
    let (commands, receiver) = mpsc::channel(QUEUE_CAPACITY);
    let (sender, events) = mpsc::channel(64);
    let (cancel, cancelled) = watch::channel(false);
    (
        RuntimeClient {
            commands,
            events,
            cancel,
        },
        RuntimeEndpoint {
            commands: receiver,
            events: sender,
            cancel: cancelled,
        },
    )
}

#[derive(Debug)]
pub struct Cancelled;
impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("task cancelled; completed changes are preserved")
    }
}
impl std::error::Error for Cancelled {}

#[derive(Debug, Clone)]
pub struct PendingInput {
    pub id: String,
    pub text: String,
    pub queued: bool,
}
/// Central projection used by the runtime and by clients displaying a resumed queue.
pub fn pending_inputs(events: &[Event]) -> Vec<PendingInput> {
    let mut pending = Vec::new();
    for event in events {
        match event {
            Event::InputSubmitted {
                id, text, queued, ..
            } => {
                if !pending.iter().any(|m: &PendingInput| &m.id == id) {
                    pending.push(PendingInput {
                        id: id.clone(),
                        text: text.clone(),
                        queued: *queued,
                    });
                }
            }
            Event::InputApplied { id, .. } => pending.retain(|m| &m.id != id),
            _ => {}
        }
    }
    pending
}

enum Transport<'a> {
    Callbacks {
        approver: &'a mut dyn FnMut(&ApprovalRequest) -> bool,
        observer: &'a mut dyn FnMut(AgentEvent),
    },
    Connected(RuntimeEndpoint),
}
pub(crate) struct RunIo<'a> {
    transport: Transport<'a>,
    pending: VecDeque<PendingInput>,
    seen: HashMap<String, (String, bool)>,
    pub(crate) starting: Option<PendingInput>,
}
impl<'a> RunIo<'a> {
    pub(crate) fn callbacks(
        approver: &'a mut dyn FnMut(&ApprovalRequest) -> bool,
        observer: &'a mut dyn FnMut(AgentEvent),
    ) -> Self {
        Self {
            transport: Transport::Callbacks { approver, observer },
            pending: VecDeque::new(),
            seen: HashMap::new(),
            starting: None,
        }
    }
    fn connected(endpoint: RuntimeEndpoint, session: &SessionLog) -> Result<Self> {
        let events = load(session.path())?.events;
        let pending = pending_inputs(&events).into();
        let seen = events
            .iter()
            .filter_map(|e| match e {
                Event::InputSubmitted {
                    id, text, queued, ..
                } => Some((id.clone(), (text.clone(), *queued))),
                _ => None,
            })
            .collect();
        Ok(Self {
            transport: Transport::Connected(endpoint),
            pending,
            seen,
            starting: None,
        })
    }
    async fn emit(&mut self, event: RuntimeEvent) -> Result<()> {
        match &mut self.transport {
            Transport::Callbacks { observer, .. } => {
                if let RuntimeEvent::Agent(event) = event {
                    observer(event);
                }
            }
            Transport::Connected(endpoint) => {
                if *endpoint.cancel.borrow() {
                    return Err(Cancelled.into());
                }
                tokio::select! {
                    biased;
                    _ = endpoint.cancel.changed() => return Err(Cancelled.into()),
                    result = endpoint.events.send(event) => result.map_err(|_| Cancelled)?,
                }
            }
        }
        Ok(())
    }
    pub(crate) async fn observe(&mut self, event: AgentEvent) -> Result<()> {
        self.emit(RuntimeEvent::Agent(event)).await
    }
    async fn accept(&mut self, session: &SessionLog, command: RuntimeCommand) -> Result<()> {
        let RuntimeCommand::Submit { id, text, queued } = command else {
            return self
                .emit(RuntimeEvent::Notice("stale approval ignored".into()))
                .await;
        };
        if let Some(previous) = self.seen.get(&id) {
            if previous != &(text, queued) {
                return self
                    .emit(RuntimeEvent::InputRejected {
                        id,
                        reason: "ID already used for different content".into(),
                    })
                    .await;
            }
            let applied = !self.pending.iter().any(|m| m.id == id);
            return self
                .emit(RuntimeEvent::Input {
                    id,
                    queued,
                    applied,
                })
                .await;
        }
        if id.is_empty()
            || id.len() > 128
            || text.trim().is_empty()
            || text.len() > MAX_MESSAGE_BYTES
            || self.pending.len() >= QUEUE_CAPACITY
        {
            return self
                .emit(RuntimeEvent::InputRejected {
                    id,
                    reason: "queue full or invalid/oversized message (64 KiB limit)".into(),
                })
                .await;
        }
        session.append(&Event::InputSubmitted {
            id: id.clone(),
            text: text.clone(),
            queued,
            ts: now_ms(),
        })?;
        self.seen.insert(id.clone(), (text.clone(), queued));
        self.pending.push_back(PendingInput {
            id: id.clone(),
            text,
            queued,
        });
        self.emit(RuntimeEvent::Input {
            id,
            queued,
            applied: false,
        })
        .await
    }
    /// Poll control messages while an external effect waits. Dropping the effect
    /// on cancellation also drops its process-group guard/provider stream.
    pub(crate) async fn wait<F: Future>(
        &mut self,
        session: &SessionLog,
        future: F,
    ) -> Result<F::Output> {
        tokio::pin!(future);
        loop {
            let command = match &mut self.transport {
                Transport::Callbacks { .. } => return Ok(future.await),
                Transport::Connected(endpoint) => {
                    if *endpoint.cancel.borrow() {
                        return Err(Cancelled.into());
                    }
                    tokio::select! {
                        biased;
                        _ = endpoint.cancel.changed() => return Err(Cancelled.into()),
                        command = endpoint.commands.recv() => command.ok_or(Cancelled)?,
                        result = &mut future => return Ok(result),
                    }
                }
            };
            self.accept(session, command).await?;
        }
    }
    pub(crate) async fn approve(
        &mut self,
        session: &SessionLog,
        call_id: &str,
        request: ApprovalRequest,
    ) -> Result<bool> {
        if let Transport::Callbacks { approver, .. } = &mut self.transport {
            return Ok(approver(&request));
        }
        static NEXT_APPROVAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let request_id = format!(
            "{}:{}:{}",
            session.id(),
            now_ms(),
            NEXT_APPROVAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        self.emit(RuntimeEvent::Approval {
            request_id: request_id.clone(),
            call_id: call_id.into(),
            request,
        })
        .await?;
        loop {
            let Transport::Connected(endpoint) = &mut self.transport else {
                unreachable!()
            };
            let command = tokio::select! {
                biased;
                _ = endpoint.cancel.changed() => return Err(Cancelled.into()),
                command = endpoint.commands.recv() => command.ok_or(Cancelled)?,
            };
            match command {
                RuntimeCommand::Approve {
                    request_id: id,
                    allowed,
                } if id == request_id => return Ok(allowed),
                other => self.accept(session, other).await?,
            }
        }
    }
    async fn drain(&mut self, session: &SessionLog) -> Result<()> {
        loop {
            let command = match &mut self.transport {
                Transport::Callbacks { .. } => None,
                Transport::Connected(endpoint) => {
                    if *endpoint.cancel.borrow() {
                        return Err(Cancelled.into());
                    }
                    match endpoint.commands.try_recv() {
                        Ok(command) => Some(command),
                        Err(mpsc::error::TryRecvError::Disconnected) => {
                            return Err(Cancelled.into())
                        }
                        Err(mpsc::error::TryRecvError::Empty) => None,
                    }
                }
            };
            match command {
                Some(command) => self.accept(session, command).await?,
                None => return Ok(()),
            }
        }
    }
    pub(crate) async fn inject(
        &mut self,
        session: &SessionLog,
        messages: &mut Vec<ChatMessage>,
    ) -> Result<bool> {
        self.drain(session).await?;
        let mut injected = false;
        while let Some(index) = self.pending.iter().position(|m| !m.queued) {
            let input = self.pending[index].clone();
            session.append(&Event::InputApplied {
                id: input.id.clone(),
                text: input.text.clone(),
                ts: now_ms(),
            })?;
            self.pending.remove(index);
            messages.push(ChatMessage::user(input.text));
            injected = true;
            self.emit(RuntimeEvent::Input {
                id: input.id,
                queued: false,
                applied: true,
            })
            .await?;
        }
        Ok(injected)
    }
    pub(crate) async fn begin(
        &mut self,
        session: &SessionLog,
        prompt: &str,
        attachments: &[niucode_core::AttachmentRef],
    ) -> Result<()> {
        if let Some(input) = self.starting.take() {
            session.append(&Event::InputApplied {
                id: input.id.clone(),
                text: input.text,
                ts: now_ms(),
            })?;
            self.pending.retain(|p| p.id != input.id);
            self.emit(RuntimeEvent::Input {
                id: input.id,
                queued: input.queued,
                applied: true,
            })
            .await
        } else {
            session.append(&Event::UserMessage {
                text: prompt.into(),
                attachments: attachments.to_vec(),
                ts: now_ms(),
            })
        }
    }
}

pub async fn run_interactive(
    provider: &dyn Provider,
    session: &SessionLog,
    config: &AgentConfig,
    prompt: &str,
    history: Vec<ChatMessage>,
    endpoint: RuntimeEndpoint,
) -> Result<RunOutcome> {
    let mut io = RunIo::connected(endpoint, session)?;
    let mut prompt = prompt.to_string();
    let mut history = history;
    loop {
        let outcome = execute_task(
            provider,
            session,
            config,
            &prompt,
            history,
            Vec::new(),
            None,
            &mut io,
        )
        .await?;
        io.drain(session).await?;
        // Submissions arriving after the final boundary become the next task.
        if let Some(next) = io.pending.front().cloned() {
            prompt = next.text.clone();
            io.starting = Some(next);
            history = crate::project_history(&load(session.path())?.events)?;
        } else {
            return Ok(outcome);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_task(
    provider: &dyn Provider,
    session: &SessionLog,
    config: &AgentConfig,
    prompt: &str,
    history: Vec<ChatMessage>,
    attachments: Vec<niucode_core::AttachmentRef>,
    store: Option<&niucode_attachments::AttachmentStore>,
    io: &mut RunIo<'_>,
) -> Result<RunOutcome> {
    let recorded = load(session.path())?.events;
    let mut known: std::collections::BTreeSet<String> = crate::changes::recorded_bodies(&recorded)
        .keys()
        .take(32_768)
        .map(|hash| (*hash).to_owned())
        .collect();
    let id = format!("{}:task:{}", session.id(), recorded.len() + 1);
    drop(recorded);
    session.append(&Event::TaskStarted {
        id: id.clone(),
        cwd: config.working_dir.display().to_string(),
        ts: now_ms(),
    })?;
    let result = async {
        io.begin(session, prompt, &attachments).await?;
        let mut before = io
            .wait(
                session,
                crate::changes::capture(&config.working_dir, session.path(), true),
            )
            .await?;
        crate::changes::deduplicate(&mut before, &mut known);
        session.append(&Event::TaskSnapshot {
            task_id: id.clone(),
            phase: "before".into(),
            snapshot: before,
            ts: now_ms(),
        })?;
        crate::run_engine(
            provider,
            session,
            config,
            prompt,
            history,
            &attachments,
            store,
            io,
        )
        .await
    }
    .await;
    // Effects are already stopped here. Cleanup evidence is bounded even after
    // cancellation, so completed writes remain reviewable without replaying them.
    let mut after = match tokio::time::timeout(
        if result.as_ref().is_err_and(|e| e.is::<Cancelled>()) {
            std::time::Duration::from_millis(100)
        } else {
            std::time::Duration::from_secs(3)
        },
        crate::changes::capture(&config.working_dir, session.path(), true),
    )
    .await
    {
        Ok(snapshot) => snapshot,
        Err(_) => niucode_core::WorkspaceSnapshot {
            complete: false,
            files: Vec::new(),
            notes: vec!["final snapshot deadline exceeded".into()],
        },
    };
    crate::changes::deduplicate(&mut after, &mut known);
    session.append(&Event::TaskSnapshot {
        task_id: id.clone(),
        phase: "after".into(),
        snapshot: after,
        ts: now_ms(),
    })?;
    let status = match &result {
        Ok(_) => "complete",
        Err(e) if e.is::<Cancelled>() => "cancelled",
        Err(_) => "error",
    };
    session.append(&Event::TaskFinished {
        id: id.clone(),
        status: status.into(),
        ts: now_ms(),
    })?;
    if let Ok(outcome) = &result {
        io.emit(RuntimeEvent::TaskDone {
            id,
            outcome: outcome.clone(),
        })
        .await?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reused_provider_call_ids_do_not_reuse_approval_authority() {
        let directory = tempfile::tempdir().unwrap();
        let session = SessionLog::create(directory.path()).unwrap();
        let (mut client, endpoint) = runtime_channel();
        let mut io = RunIo::connected(endpoint, &session).unwrap();
        let request = ApprovalRequest {
            tool: "write".into(),
            summary: "write file".into(),
            preview: None,
        };
        let runtime = async {
            assert!(io
                .approve(&session, "reused-call", request.clone())
                .await
                .unwrap());
            assert!(!io.approve(&session, "reused-call", request).await.unwrap());
        };
        let driver = async {
            let Some(RuntimeEvent::Approval {
                request_id: first, ..
            }) = client.events.recv().await
            else {
                panic!("approval");
            };
            client
                .commands
                .send(RuntimeCommand::Approve {
                    request_id: first.clone(),
                    allowed: true,
                })
                .await
                .unwrap();
            let Some(RuntimeEvent::Approval {
                request_id: second, ..
            }) = client.events.recv().await
            else {
                panic!("approval");
            };
            assert_ne!(first, second);
            client
                .commands
                .send(RuntimeCommand::Approve {
                    request_id: first,
                    allowed: true,
                })
                .await
                .unwrap();
            assert!(matches!(
                client.events.recv().await,
                Some(RuntimeEvent::Notice(_))
            ));
            client
                .commands
                .send(RuntimeCommand::Approve {
                    request_id: second,
                    allowed: false,
                })
                .await
                .unwrap();
            // Keep the client alive until the matching decision is consumed.
            client
        };
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            tokio::join!(runtime, driver)
        })
        .await
        .unwrap();
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn failed_input_append_is_never_acknowledged_or_queued() {
        let directory = tempfile::tempdir().unwrap();
        let session = SessionLog::create(directory.path()).unwrap();
        let (mut client, endpoint) = runtime_channel();
        let mut io = RunIo::connected(endpoint, &session).unwrap();
        let failed_log = SessionLog::open("/dev/full").unwrap();
        assert!(io
            .accept(
                &failed_log,
                RuntimeCommand::Submit {
                    id: "unwritten".into(),
                    text: "keep this".into(),
                    queued: true
                }
            )
            .await
            .is_err());
        assert!(io.pending.is_empty());
        assert!(client.events.try_recv().is_err());
    }

    #[tokio::test]
    async fn cancel_bypasses_a_full_event_channel() {
        let directory = tempfile::tempdir().unwrap();
        let session = SessionLog::create(directory.path()).unwrap();
        let (client, endpoint) = runtime_channel();
        let mut io = RunIo::connected(endpoint, &session).unwrap();
        for _ in 0..64 {
            io.emit(RuntimeEvent::Notice("occupied".into()))
                .await
                .unwrap();
        }
        let blocked = io.emit(RuntimeEvent::Notice("no space".into()));
        let cancel = async {
            tokio::task::yield_now().await;
            client.cancel();
        };
        let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(1), async {
            tokio::join!(blocked, cancel)
        })
        .await
        .unwrap();
        assert!(result.unwrap_err().is::<Cancelled>());
    }

    #[tokio::test]
    async fn invalid_submission_is_rejected_without_durable_acceptance() {
        let directory = tempfile::tempdir().unwrap();
        let session = SessionLog::create(directory.path()).unwrap();
        let (mut client, endpoint) = runtime_channel();
        let mut io = RunIo::connected(endpoint, &session).unwrap();
        io.accept(
            &session,
            RuntimeCommand::Submit {
                id: "large".into(),
                text: "x".repeat(MAX_MESSAGE_BYTES + 1),
                queued: false,
            },
        )
        .await
        .unwrap();
        assert!(
            matches!(client.events.recv().await, Some(RuntimeEvent::InputRejected { id, .. }) if id == "large")
        );
        assert!(load(session.path()).unwrap().events.is_empty());
    }
}
