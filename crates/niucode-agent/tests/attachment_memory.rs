//! C-layer evidence: an attachment survives a store reopen and a new session.

use niucode_agent::{
    project_history_with_attachments, run_with_attachments, run_with_history, AgentConfig,
    ApprovalMode, ApprovalRequest,
};
use niucode_attachments::AttachmentStore;
use niucode_core::{load, SessionLog};
use niucode_provider::{MessageContent, ScriptedProvider, StreamEvent};
use niucode_tools::{NetworkMode, SandboxMode, SandboxPolicy};

fn config(directory: &std::path::Path) -> AgentConfig {
    let mut config = AgentConfig::new("replay", directory);
    config.provider = "replay".into();
    config.approval = ApprovalMode::Never;
    config.sandbox = SandboxPolicy::new(SandboxMode::ReadOnly, NetworkMode::Off);
    config
}

fn image_parts(history: &[niucode_provider::ChatMessage]) -> usize {
    history
        .iter()
        .filter(|message| message.has_images())
        .filter(|message| {
            matches!(
                message.content,
                Some(MessageContent::Parts(ref parts))
                    if parts.iter().any(|part| matches!(part, niucode_provider::ContentPart::ImageUrl { .. }))
            )
        })
        .count()
}

#[tokio::test]
async fn recent_image_survives_store_reopen_and_a_new_session() {
    let directory = tempfile::tempdir().unwrap();
    let store = AttachmentStore::open(directory.path().join("attachments")).unwrap();
    let image = store
        .put(
            b"\x89PNG\r\n\x1a\nimage",
            Some("shot.png"),
            Some("test image"),
        )
        .unwrap();
    let mut log = SessionLog::create(directory.path()).unwrap();
    let config = config(directory.path());
    let provider = ScriptedProvider::from_turns(vec![vec![
        StreamEvent::TextDelta("seen".into()),
        StreamEvent::Done {
            finish_reason: Some("stop".into()),
        },
    ]]);
    run_with_attachments(
        &provider,
        &mut log,
        &config,
        "look at this",
        vec![image.clone()],
        &store,
        &mut |_request: &ApprovalRequest| false,
        &mut |_event| {},
    )
    .await
    .expect("first turn");
    drop(store);

    let reopened = AttachmentStore::open(directory.path().join("attachments")).unwrap();
    let loaded = load(log.path()).expect("load");
    let history = project_history_with_attachments(&loaded.events, &reopened, 2).expect("project");
    assert_eq!(image_parts(&history), 1);
    let data = history
        .iter()
        .filter_map(|message| message.content.as_ref())
        .find_map(|content| match content {
            MessageContent::Parts(parts) => parts.iter().find_map(|part| match part {
                niucode_provider::ContentPart::ImageUrl { image_url } => {
                    Some(image_url.url.as_str())
                }
                _ => None,
            }),
            MessageContent::Text(_) => None,
        });
    assert!(data.is_some_and(|url| url.starts_with("data:image/png;base64,")));

    // A fresh session starts from the resumed history; the image is still in
    // the provider request, without re-reading the original file.
    let mut next = SessionLog::create(directory.path()).unwrap();
    let provider = ScriptedProvider::from_turns(vec![vec![
        StreamEvent::TextDelta("still visible".into()),
        StreamEvent::Done {
            finish_reason: Some("stop".into()),
        },
    ]]);
    run_with_history(
        &provider,
        &mut next,
        &config,
        "what was the image?",
        history,
        &mut |_request: &ApprovalRequest| false,
        &mut |_event| {},
    )
    .await
    .expect("second turn");
    let request = provider.requests();
    assert_eq!(request.len(), 1);
    assert!(
        request[0]
            .messages
            .iter()
            .any(|message| message.has_images()),
        "resumed request lost the image part"
    );
}
