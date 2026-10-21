//! Recorded history is read on demand; full bodies are not a second session cache.
use crate::text_layout::wrap;
use anyhow::Result;
use niucode_core::Event;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub struct Details {
    path: PathBuf,
    entries: Vec<(u64, String)>,
    selected: usize,
    pub offset: usize,
    pub title: String,
    pub lines: Vec<String>,
    width: usize,
}
impl Details {
    pub fn open(path: &Path, width: usize) -> Result<Self> {
        let mut reader = BufReader::new(File::open(path)?);
        let mut entries = Vec::new();
        let mut line = String::new();
        let mut ordinal = 0;
        loop {
            let offset = reader.stream_position()?;
            line.clear();
            if reader.read_line(&mut line)? == 0 {
                break;
            }
            let Ok(event) = serde_json::from_str::<Event>(&line) else {
                continue;
            };
            ordinal += 1;
            let label = match &event {
                Event::TaskSnapshot { phase, .. } if phase == "after" => {
                    Some("task changes / command evidence".into())
                }
                Event::ToolResult { call_id, .. } => Some(format!("tool {call_id}")),
                Event::AssistantMessage {
                    reasoning, text, ..
                } if reasoning.is_some() || !text.is_empty() => {
                    Some("assistant / reasoning".into())
                }
                Event::UserMessage { .. } | Event::InputApplied { .. } => Some("user".into()),
                _ => None,
            };
            if let Some(label) = label {
                entries.push((offset, format!("e{ordinal:06} · {label}")));
            }
        }
        let selected = entries.len().saturating_sub(1);
        let mut result = Self {
            path: path.into(),
            entries,
            selected,
            offset: 0,
            title: String::new(),
            lines: Vec::new(),
            width,
        };
        result.refresh(width)?;
        Ok(result)
    }
    pub fn refresh(&mut self, width: usize) -> Result<()> {
        self.width = width;
        let Some((offset, title)) = self.entries.get(self.selected) else {
            self.title = "No recorded details".into();
            self.lines =
                vec!["In-flight reasoning is available after the response is recorded.".into()];
            return Ok(());
        };
        let mut reader = BufReader::new(File::open(&self.path)?);
        reader.seek(SeekFrom::Start(*offset))?;
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let event: Event = serde_json::from_str(&line)?;
        let body = match event {
            Event::TaskSnapshot { task_id, .. } => {
                let events = niucode_core::load(&self.path)?.events;
                niucode_agent::task_review_by_id(&events, &task_id)
                    .map(|review| crate::presentation::evidence(&review))
                    .unwrap_or_else(|| "Task review unavailable".into())
            }
            Event::ToolResult { output, .. } => output,
            Event::AssistantMessage {
                text, reasoning, ..
            } => format!(
                "Reasoning\n{}\n\nAnswer\n{text}",
                reasoning.as_deref().unwrap_or("(none recorded)")
            ),
            Event::UserMessage { text, .. } | Event::InputApplied { text, .. } => text,
            _ => String::new(),
        };
        self.title = format!(
            "{}/{} · {title} · [/] previous/next · ↑↓ scroll · Esc close",
            self.selected + 1,
            self.entries.len()
        );
        self.lines = wrap(&body, width);
        self.offset = self.offset.min(self.lines.len().saturating_sub(1));
        Ok(())
    }
    pub fn navigate(&mut self, delta: isize) -> Result<()> {
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.entries.len().saturating_sub(1));
        self.offset = 0;
        self.refresh(self.width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_details_reflow_without_blank_rows_or_changing_the_log() {
        let root = std::env::temp_dir().join(format!(
            "niucode-details-{}-{}",
            std::process::id(),
            niucode_core::now_ms()
        ));
        let session = niucode_core::SessionLog::create(&root).unwrap();
        session
            .append(&Event::ToolResult {
                call_id: "read".into(),
                ok: true,
                output: "abcd\n\n中👩‍💻\n\x1b[31m".into(),
                ts: 1,
            })
            .unwrap();
        let original = std::fs::read(session.path()).unwrap();
        let mut details = Details::open(session.path(), 4).unwrap();
        assert_eq!(details.lines, ["abcd", "", "中👩‍💻", "␛[31", "m"]);
        details.offset = 999;
        details.refresh(2).unwrap();
        assert_eq!(details.lines, ["ab", "cd", "", "中", "👩‍💻", "␛[", "31", "m"]);
        assert_eq!(details.offset, details.lines.len() - 1);
        assert_eq!(std::fs::read(session.path()).unwrap(), original);
        drop(session);
        std::fs::remove_dir_all(root).unwrap();
    }
}
