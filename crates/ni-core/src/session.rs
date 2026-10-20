//! Append-only JSONL session storage.
//!
//! A session is a file of one JSON event per line. Appends are flushed
//! immediately; loading tolerates a corrupt tail instead of discarding the
//! whole session.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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
    writer: BufWriter<File>,
}

impl SessionLog {
    /// Create a new session file under `dir`.
    pub fn create(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)
            .with_context(|| format!("create session directory {}", dir.display()))?;

        let seq = SESSION_SEQ.fetch_add(1, Ordering::Relaxed);
        let id = format!("{}-{}-{}", now_ms(), std::process::id(), seq);
        let path = dir.join(format!("{id}.jsonl"));
        let file = OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("create session file {}", path.display()))?;

        Ok(Self {
            id,
            path,
            writer: BufWriter::new(file),
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one event and flush it to disk.
    pub fn append(&mut self, event: &Event) -> Result<()> {
        serde_json::to_writer(&mut self.writer, event).context("serialize session event")?;
        self.writer
            .write_all(b"\n")
            .context("write session event")?;
        self.writer.flush().context("flush session event")?;
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
        let dir = std::env::temp_dir().join(format!("ni-core-{tag}-{}-{n}", now_ms()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn round_trip_preserves_events() {
        let dir = temp_dir("roundtrip");
        let mut log = SessionLog::create(&dir).expect("create log");
        let ts = now_ms();

        let events = vec![
            Event::SessionStart {
                id: log.id().to_string(),
                cwd: "/tmp/project".into(),
                model: "test-model".into(),
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
