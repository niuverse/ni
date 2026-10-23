//! Offline session replay and fork.
//!
//! Replay is a pure projection over decoded events; it never constructs a
//! provider. Fork copies a validated event prefix into a new append-only file
//! and appends a provenance event.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use niucode_agent::{estimate_context, project_history};
use niucode_attachments::AttachmentStore;
use niucode_core::{load, now_ms, Event, EventId, SessionLog};
use niucode_provider::ChatMessage;
use niucode_tools::Toolbox;

use crate::text_layout::clean_text;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayTurn {
    pub number: usize,
    pub event: EventId,
    pub user: String,
    /// Number of decoded events projected for this request.
    pub context_events: usize,
    pub messages: usize,
    pub tokens: usize,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayReport {
    pub session: PathBuf,
    pub events: usize,
    pub warnings: Vec<String>,
    pub turns: Vec<ReplayTurn>,
    pub compactions: usize,
    pub instructions: usize,
    pub final_messages: usize,
    pub final_tokens: usize,
    pub final_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkReport {
    pub parent: PathBuf,
    pub destination: PathBuf,
    pub at: EventId,
    pub copied: usize,
    pub attachments: usize,
    pub warnings: Vec<String>,
}

/// Rebuild the model projection at every user turn without provider calls.
pub fn replay(path: &Path) -> Result<ReplayReport> {
    let loaded = load(path)?;
    let events = &loaded.events;
    let tools = Toolbox::specs();
    let mut turns = Vec::new();
    let mut compactions = 0_usize;
    let mut instructions = 0_usize;

    for (index, event) in events.iter().enumerate() {
        match event {
            Event::Compaction { .. } => compactions += 1,
            Event::Instructions { .. } => instructions += 1,
            _ => {}
        }
        if !matches!(event, Event::UserMessage { .. }) {
            continue;
        }

        // Compaction and instruction events are appended after the user
        // message and before the next provider request. Include them, but stop
        // before the assistant response.
        let end = events
            .iter()
            .enumerate()
            .skip(index + 1)
            .find(|(_, candidate)| matches!(candidate, Event::AssistantMessage { .. }))
            .map(|(assistant, _)| assistant)
            .unwrap_or(events.len());
        let context = project_history(&events[..end])?;
        let tokens = estimate_context(&context, &tools);
        turns.push(ReplayTurn {
            number: turns.len() + 1,
            event: EventId::from_index(index),
            user: one_line(text_of(event), 120),
            context_events: end,
            messages: context.len(),
            tokens,
            digest: digest_messages(&context),
        });
    }

    let final_context = project_history(events)?;
    let final_tokens = estimate_context(&final_context, &tools);
    Ok(ReplayReport {
        session: path.to_path_buf(),
        events: events.len(),
        warnings: loaded.warnings,
        turns,
        compactions,
        instructions,
        final_messages: final_context.len(),
        final_tokens,
        final_digest: digest_messages(&final_context),
    })
}

/// Copy the validated prefix through `at` into a new session file.
pub fn fork(source: &Path, at: &str, destination_dir: &Path) -> Result<ForkReport> {
    let at = EventId::parse(at).map_err(|message| anyhow::anyhow!("{message}"))?;
    let loaded = load(source)?;
    let index = at.index();
    if index >= loaded.events.len() {
        bail!(
            "event {at} does not exist; {} has {} decoded events",
            source.display(),
            loaded.events.len()
        );
    }

    let destination = SessionLog::create(destination_dir)?;
    for event in &loaded.events[..=index] {
        destination.append(event)?;
    }
    destination.append(&Event::Fork {
        parent: source.display().to_string(),
        parent_event: at.to_string(),
        ts: now_ms(),
    })?;
    let mut warnings = loaded.warnings;
    let attachments = copy_attachments(
        &loaded.events[..=index],
        source,
        destination_dir,
        &mut warnings,
    )?;

    Ok(ForkReport {
        parent: source.to_path_buf(),
        destination: destination.path().to_path_buf(),
        at,
        copied: index + 1,
        attachments,
        warnings,
    })
}

fn copy_attachments(
    events: &[Event],
    source: &Path,
    destination_dir: &Path,
    warnings: &mut Vec<String>,
) -> Result<usize> {
    let Some(source_root) = source.parent().map(|parent| parent.join("attachments")) else {
        return Ok(0);
    };
    if !source_root.exists() {
        return Ok(0);
    }
    let destination_root = destination_dir.join("attachments");
    if source_root == destination_root {
        return Ok(0);
    }
    let source_label = source_root.display().to_string();
    let source_store = AttachmentStore::open(source_root)?;
    let destination_store = AttachmentStore::open(destination_root)?;
    let mut seen = std::collections::HashSet::new();
    let mut copied = 0;
    for event in events {
        let Event::UserMessage { attachments, .. } = event else {
            continue;
        };
        for reference in attachments {
            if !seen.insert(reference.sha256.clone()) {
                continue;
            }
            match source_store.load(reference) {
                Ok(bytes) => {
                    destination_store.put(
                        &bytes,
                        reference.name.as_deref(),
                        reference.alt.as_deref(),
                    )?;
                    copied += 1;
                }
                Err(error) => warnings.push(format!(
                    "attachment {} missing from {}: {error}",
                    reference.sha256, source_label
                )),
            }
        }
    }
    Ok(copied)
}

fn text_of(event: &Event) -> &str {
    match event {
        Event::UserMessage { text, .. } => text,
        _ => "",
    }
}

fn one_line(text: &str, max_chars: usize) -> String {
    let cleaned = clean_text(text);
    let mut rendered = String::new();
    for (index, character) in cleaned.chars().enumerate() {
        if index >= max_chars {
            rendered.push('…');
            break;
        }
        rendered.push(match character {
            '\n' | '\r' | '\t' => ' ',
            other => other,
        });
    }
    rendered.trim().to_string()
}

fn digest_messages(messages: &[ChatMessage]) -> String {
    let bytes = serde_json::to_vec(messages).unwrap_or_default();
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let mut rendered = String::new();
    let _ = write!(&mut rendered, "{hash:016x}");
    rendered
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use niucode_core::Usage;

    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("niucode-fork-{tag}-{stamp}-{sequence}"));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create temp dir");
        directory
    }

    fn start(id: &str) -> Event {
        Event::SessionStart {
            id: id.into(),
            cwd: "/tmp".into(),
            model: "test".into(),
            provider: "deepseek".into(),
            ts: 1,
        }
    }

    fn user(text: &str) -> Event {
        Event::UserMessage {
            text: text.into(),
            attachments: Vec::new(),
            ts: 2,
        }
    }

    fn assistant(text: &str) -> Event {
        Event::AssistantMessage {
            text: text.into(),
            reasoning: None,
            tool_calls: Vec::new(),
            ts: 3,
        }
    }

    fn write_session(directory: &Path, events: &[Event]) -> PathBuf {
        let log = SessionLog::create(directory).expect("session");
        for event in events {
            log.append(event).expect("append");
        }
        log.path().to_path_buf()
    }

    #[test]
    fn replay_includes_compaction_appended_after_the_user_message() {
        let directory = temp_dir("replay");
        let path = write_session(
            &directory,
            &[
                start("s"),
                user("first request"),
                Event::Compaction {
                    summary: "HANDOFF".into(),
                    upto: 1,
                    before_tokens: 100,
                    after_tokens: 20,
                    ts: 3,
                },
                assistant("answer"),
            ],
        );

        let report = replay(&path).expect("replay");
        assert_eq!(report.turns.len(), 1);
        let turn = &report.turns[0];
        assert_eq!(turn.event.to_string(), "e000002");
        assert_eq!(turn.context_events, 3);
        assert_eq!(turn.messages, 2);
        assert!(turn.tokens > 0);
        assert_eq!(report.final_messages, 3);
        assert!(report.final_digest.len() == 16);

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn fork_copies_the_prefix_and_leaves_the_parent_untouched() {
        let directory = temp_dir("fork");
        let source = write_session(
            &directory,
            &[start("parent"), user("keep this"), assistant("done")],
        );
        let before = fs::read(&source).expect("read before");
        let forks = directory.join("forks");

        let report = fork(&source, "e000002", &forks).expect("fork");
        assert_eq!(report.copied, 2);
        assert_eq!(report.at.to_string(), "e000002");
        assert_eq!(fs::read(&source).expect("read after"), before);

        let loaded = load(&report.destination).expect("load fork");
        assert_eq!(loaded.events.len(), 3);
        assert_eq!(loaded.events[0], start("parent"));
        assert_eq!(loaded.events[1], user("keep this"));
        assert!(matches!(
            &loaded.events[2],
            Event::Fork {
                parent,
                parent_event,
                ..
            } if parent == &source.display().to_string() && parent_event == "e000002"
        ));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn fork_copies_referenced_attachments() {
        let directory = temp_dir("fork-attachments");
        let store = AttachmentStore::open(directory.join("attachments")).unwrap();
        let image = store
            .put(b"\x89PNG\r\n\x1a\nimage", Some("shot.png"), Some("screen"))
            .unwrap();
        let source = write_session(
            &directory,
            &[
                start("parent"),
                Event::UserMessage {
                    text: "look".into(),
                    attachments: vec![image.clone()],
                    ts: 2,
                },
                assistant("done"),
            ],
        );
        let forks = directory.join("forks");
        let report = fork(&source, "e000002", &forks).expect("fork");
        assert_eq!(report.attachments, 1);
        let destination_store = AttachmentStore::open(forks.join("attachments")).unwrap();
        assert_eq!(
            destination_store.load(&image).unwrap(),
            b"\x89PNG\r\n\x1a\nimage"
        );
        let loaded = load(&report.destination).unwrap();
        let history =
            niucode_agent::project_history_with_attachments(&loaded.events, &destination_store, 2)
                .unwrap();
        assert!(history.iter().any(|message| message.has_images()));
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn fork_preserves_compaction_cutoffs() {
        let directory = temp_dir("fork-compaction");
        let source = write_session(
            &directory,
            &[
                start("parent"),
                user("first"),
                assistant("first answer"),
                Event::Compaction {
                    summary: "HANDOFF".into(),
                    upto: 1,
                    before_tokens: 100,
                    after_tokens: 20,
                    ts: 4,
                },
                user("second"),
                assistant("second answer"),
            ],
        );
        let report = fork(&source, "e000006", &directory).expect("fork");
        let loaded = load(&report.destination).expect("load");
        let source_loaded = load(&source).expect("load source");
        assert_eq!(
            project_history(&loaded.events[..6]).expect("fork projection"),
            project_history(&source_loaded.events[..6]).expect("source projection")
        );
        assert_eq!(
            project_history(&loaded.events).expect("full fork projection"),
            project_history(&source_loaded.events).expect("full source projection"),
            "the trailing fork event must not change the projection"
        );
        assert!(loaded
            .events
            .iter()
            .any(|event| matches!(event, Event::Fork { .. })));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn fork_rejects_unknown_event_ids() {
        let directory = temp_dir("missing");
        let source = write_session(&directory, &[start("parent"), user("one")]);
        let error = fork(&source, "e000009", &directory).expect_err("must fail");
        assert!(error.to_string().contains("does not exist"));

        let error = fork(&source, "nope", &directory).expect_err("must fail");
        assert!(error.to_string().contains("invalid event id"));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn fork_skips_a_corrupt_line_and_reports_it() {
        let directory = temp_dir("corrupt");
        let source = directory.join("corrupt.jsonl");
        let good_one = serde_json::to_string(&start("parent")).expect("serialize");
        let good_two = serde_json::to_string(&user("second valid")).expect("serialize");
        fs::write(&source, format!("{good_one}\n{{ not json }}\n{good_two}\n")).expect("write");

        let report = fork(&source, "e000002", &directory).expect("fork");
        assert_eq!(report.copied, 2);
        assert_eq!(report.warnings.len(), 1);
        let loaded = load(&report.destination).expect("load fork");
        assert_eq!(loaded.events.len(), 3);
        assert_eq!(loaded.events[1], user("second valid"));
        assert!(matches!(loaded.events[2], Event::Fork { .. }));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn replay_uses_provider_usage_only_from_the_log() {
        let directory = temp_dir("usage");
        let path = write_session(
            &directory,
            &[
                start("s"),
                user("one"),
                assistant("one answer"),
                Event::TurnEnd {
                    finish_reason: None,
                    usage: Usage {
                        input_tokens: 10,
                        cached_tokens: 5,
                        ..Usage::default()
                    },
                    ts: 4,
                },
            ],
        );
        let report = replay(&path).expect("replay");
        assert_eq!(report.turns.len(), 1);
        assert_eq!(report.final_messages, 2);
        assert_eq!(report.events, 4);
        assert!(report.warnings.is_empty());

        fs::remove_dir_all(directory).expect("cleanup");
    }
}
