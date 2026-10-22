//! Append-only JSONL session storage.
//!
//! A session is a file of one JSON event per line. Appends are flushed
//! immediately; loading tolerates a corrupt tail instead of discarding the
//! whole session.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use anyhow::{Context, Result};

use crate::event::{now_ms, Event};

static SESSION_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct LoadedSession {
    pub events: Vec<Event>,
    /// Lines that could not be decoded. The rest of the session is still valid.
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct SessionLog {
    id: String,
    path: PathBuf,
    writer: Mutex<Option<BufWriter<File>>>,
}

impl SessionLog {
    /// Create a new session file under `dir`.
    pub fn create(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)
            .with_context(|| format!("create session directory {}", dir.display()))?;

        let seq = SESSION_SEQ.fetch_add(1, Ordering::Relaxed);
        let id = format!("{}-{}-{}", now_ms(), std::process::id(), seq);
        let path = dir.join(format!("{id}.jsonl"));
        let mut options = OpenOptions::new();
        options.create_new(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(&path)
            .with_context(|| format!("create session file {}", path.display()))?;

        Ok(Self {
            id,
            path,
            writer: Mutex::new(Some(BufWriter::new(file))),
        })
    }

    /// Create a session at an embedder-chosen path. Existing files are never
    /// overwritten; callers archive or move an old log first.
    pub fn create_at(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create session directory {}", parent.display()))?;
        }
        let id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("session")
            .to_string();
        let mut options = OpenOptions::new();
        options.create_new(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(&path)
            .with_context(|| format!("create session file {}", path.display()))?;
        Ok(Self {
            id,
            path,
            writer: Mutex::new(Some(BufWriter::new(file))),
        })
    }

    /// Open an existing session for appending.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .append(true)
            .open(&path)
            .with_context(|| format!("open session for append {}", path.display()))?;
        let id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("session")
            .to_string();

        Ok(Self {
            id,
            path,
            writer: Mutex::new(Some(BufWriter::new(file))),
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one event and flush it to disk.
    pub fn append(&self, event: &Event) -> Result<()> {
        // One writer, one complete serialized append. The runtime can service
        // control messages while an effect borrows the same log immutably.
        let mut bytes = serde_json::to_vec(event).context("serialize session event")?;
        bytes.push(b'\n');
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("session writer poisoned"))?;
        let active = writer
            .as_mut()
            .context("session writer stopped after an earlier append failure")?;
        if let Err(error) = active.write_all(&bytes).and_then(|()| active.flush()) {
            // Do not retry a possibly partial event, including in BufWriter::drop.
            if let Some(failed) = writer.take() {
                let _ = failed.into_parts();
            }
            return Err(error).context("append and flush session event");
        }
        Ok(())
    }
}

/// Read a session, reporting undecodable lines as warnings.
pub fn load(path: &Path) -> Result<LoadedSession> {
    let file = File::open(path).with_context(|| format!("open session file {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut events = Vec::new();
    let mut warnings = Vec::new();

    for (index, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("read line {}", index + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Event>(&line) {
            Ok(event) => events.push(event),
            Err(error) => warnings.push(format!("line {}: {error}", index + 1)),
        }
    }

    Ok(LoadedSession { events, warnings })
}

/// List session files in `dir`, sorted by name.
pub fn list(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for entry in
        fs::read_dir(dir).with_context(|| format!("read session directory {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "jsonl") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, ToolCall, Usage};

    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("niucode-core-{tag}-{}-{n}", now_ms()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn failed_append_disables_writer_instead_of_retrying_partial_events() {
        let log = SessionLog::open("/dev/full").unwrap();
        let event = Event::SessionEnd { ts: 1 };
        assert!(log
            .append(&event)
            .unwrap_err()
            .to_string()
            .contains("append and flush"));
        assert!(log
            .append(&event)
            .unwrap_err()
            .to_string()
            .contains("stopped after an earlier"));
    }

    #[test]
    fn round_trip_preserves_events() {
        let dir = temp_dir("roundtrip");
        let log = SessionLog::create(&dir).expect("create log");
        let ts = now_ms();

        let events = vec![
            Event::SessionStart {
                id: log.id().to_string(),
                cwd: "/tmp/project".into(),
                model: "test-model".into(),
                provider: "deepseek".into(),
                ts,
            },
            Event::UserMessage {
                text: "fix the failing test".into(),
                ts,
            },
            Event::AssistantMessage {
                text: "reading the test".into(),
                reasoning: Some("need the failure output first".into()),
                tool_calls: vec![ToolCall {
                    id: "call-1".into(),
                    name: "read".into(),
                    input: serde_json::json!({"path": "src/lib.rs"}),
                }],
                ts,
            },
            Event::ToolResult {
                call_id: "call-1".into(),
                ok: true,
                output: "fn main() {}".into(),
                ts,
            },
            Event::TurnEnd {
                finish_reason: Some("tool_calls".into()),
                usage: Usage {
                    input_tokens: 120,
                    output_tokens: 30,
                    cached_tokens: 80,
                    reasoning_tokens: 10,
                },
                ts,
            },
        ];

        for event in &events {
            log.append(event).expect("append");
        }

        let loaded = load(log.path()).expect("load");
        assert_eq!(loaded.events, events);
        assert!(loaded.warnings.is_empty());
        assert_eq!(list(&dir).expect("list"), vec![log.path().to_path_buf()]);

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn create_at_uses_the_chosen_path_and_refuses_to_overwrite() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("thread.jsonl");
        let log = SessionLog::create_at(&path).unwrap();
        assert_eq!(log.path(), path);
        assert!(path.exists());
        drop(log);
        assert!(SessionLog::create_at(&path).is_err());
    }

    #[test]
    fn created_session_files_are_private() {
        use std::os::unix::fs::PermissionsExt;

        let dir = temp_dir("private");
        let log = SessionLog::create(&dir).expect("create log");
        let mode = fs::metadata(log.path()).expect("stat").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn corrupt_tail_is_a_warning_not_a_failure() {
        let dir = temp_dir("corrupt");
        let path = dir.join("session.jsonl");
        let good = serde_json::to_string(&Event::UserMessage {
            text: "hello".into(),
            ts: 1,
        })
        .expect("serialize");
        fs::write(&path, format!("{good}\n{{ not json }}\n")).expect("write fixture");

        let loaded = load(&path).expect("load");
        assert_eq!(loaded.events.len(), 1);
        assert_eq!(loaded.warnings.len(), 1);
        assert!(loaded.warnings[0].starts_with("line 2:"));

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn reopening_appends_to_the_same_session() {
        let dir = temp_dir("reopen");
        let log = SessionLog::create(&dir).expect("create log");
        let path = log.path().to_path_buf();
        log.append(&Event::UserMessage {
            text: "first".into(),
            ts: 1,
        })
        .expect("append");
        drop(log);

        let reopened = SessionLog::open(&path).expect("reopen");
        reopened
            .append(&Event::SessionEnd { ts: 2 })
            .expect("append after reopen");

        let loaded = load(&path).expect("load");
        assert_eq!(loaded.events.len(), 2);

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn unknown_event_types_do_not_drop_readable_lines() {
        let dir = temp_dir("unknown-event");
        let path = dir.join("session.jsonl");
        let good = serde_json::to_string(&Event::SessionEnd { ts: 1 }).expect("serialize");
        let future = r#"{"type":"future_event","payload":42,"ts":2}"#;
        fs::write(&path, format!("{good}\n{future}\n")).expect("write fixture");

        let loaded = load(&path).expect("load");
        assert_eq!(loaded.events.len(), 1);
        assert_eq!(loaded.warnings.len(), 1);
        assert!(loaded.warnings[0].starts_with("line 2:"));

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn unknown_fields_on_known_events_are_ignored() {
        let dir = temp_dir("unknown-field");
        let path = dir.join("session.jsonl");
        let future = r#"{"type":"session_end","ts":1,"future_field":{"nested":true}}"#;
        fs::write(&path, format!("{future}\n")).expect("write fixture");

        let loaded = load(&path).expect("load");
        assert_eq!(loaded.events.len(), 1);
        assert!(loaded.warnings.is_empty());

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn blank_lines_are_ignored() {
        let dir = temp_dir("blank");
        let path = dir.join("session.jsonl");
        let good = serde_json::to_string(&Event::SessionEnd { ts: 1 }).expect("serialize");
        fs::write(&path, format!("\n{good}\n\n")).expect("write fixture");

        let loaded = load(&path).expect("load");
        assert_eq!(loaded.events.len(), 1);
        assert!(loaded.warnings.is_empty());

        fs::remove_dir_all(dir).expect("cleanup");
    }
}

#[cfg(test)]
mod compaction_tests {
    use crate::event::Event;

    #[test]
    fn old_compaction_events_default_token_counts() {
        let raw = r#"{"type":"compaction","summary":"handoff","upto":3,"ts":1}"#;
        let event: Event = serde_json::from_str(raw).expect("parse");
        match event {
            Event::Compaction {
                summary,
                upto,
                before_tokens,
                after_tokens,
                ..
            } => {
                assert_eq!(summary, "handoff");
                assert_eq!(upto, 3);
                assert_eq!((before_tokens, after_tokens), (0, 0));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
