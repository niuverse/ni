//! Side-effect-free change previews for approval prompts.
//!
//! The tool layer owns path resolution and knows what `write` / `edit` will
//! change, so it builds the structured diff. Clients render it; the agent loop
//! only forwards the value.

use similar::{DiffTag, TextDiff};

use crate::{input_string, Toolbox};

/// Number of unchanged context lines kept around each diff change.
pub const PREVIEW_CONTEXT_LINES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Add,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewLine {
    pub kind: LineKind,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewHunk {
    pub old_start: usize,
    pub old_len: usize,
    pub new_start: usize,
    pub new_len: usize,
    pub lines: Vec<PreviewLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePreview {
    pub path: String,
    pub is_new: bool,
    pub added: usize,
    pub removed: usize,
    pub hunks: Vec<PreviewHunk>,
}

impl FilePreview {
    /// Diff two in-memory texts. This does not touch the filesystem.
    pub fn from_text(path: impl Into<String>, old: &str, new: &str, is_new: bool) -> Self {
        let diff = TextDiff::from_lines(old, new);
        let mut hunks = Vec::new();
        let mut added = 0;
        let mut removed = 0;

        for group in diff.grouped_ops(PREVIEW_CONTEXT_LINES) {
            let Some(first) = group.first() else {
                continue;
            };
            let Some(last) = group.last() else {
                continue;
            };

            let (old_start, old_len) = header_range(first.old_range().start, last.old_range().end);
            let (new_start, new_len) = header_range(first.new_range().start, last.new_range().end);
            let mut lines = Vec::new();

            for op in &group {
                match op.tag() {
                    DiffTag::Equal => {
                        let new_range = op.new_range();
                        for (offset, old_index) in op.old_range().enumerate() {
                            lines.push(PreviewLine {
                                kind: LineKind::Context,
                                old_line: Some(old_index + 1),
                                new_line: Some(new_range.start + offset + 1),
                                text: line_text(diff.old_slices()[old_index]),
                            });
                        }
                    }
                    DiffTag::Delete => {
                        for old_index in op.old_range() {
                            removed += 1;
                            lines.push(PreviewLine {
                                kind: LineKind::Remove,
                                old_line: Some(old_index + 1),
                                new_line: None,
                                text: line_text(diff.old_slices()[old_index]),
                            });
                        }
                    }
                    DiffTag::Insert => {
                        for new_index in op.new_range() {
                            added += 1;
                            lines.push(PreviewLine {
                                kind: LineKind::Add,
                                old_line: None,
                                new_line: Some(new_index + 1),
                                text: line_text(diff.new_slices()[new_index]),
                            });
                        }
                    }
                    DiffTag::Replace => {
                        for old_index in op.old_range() {
                            removed += 1;
                            lines.push(PreviewLine {
                                kind: LineKind::Remove,
                                old_line: Some(old_index + 1),
                                new_line: None,
                                text: line_text(diff.old_slices()[old_index]),
                            });
                        }
                        for new_index in op.new_range() {
                            added += 1;
                            lines.push(PreviewLine {
                                kind: LineKind::Add,
                                old_line: None,
                                new_line: Some(new_index + 1),
                                text: line_text(diff.new_slices()[new_index]),
                            });
                        }
                    }
                }
            }

            hunks.push(PreviewHunk {
                old_start,
                old_len,
                new_start,
                new_len,
                lines,
            });
        }

        Self {
            path: path.into(),
            is_new,
            added,
            removed,
            hunks,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewBody {
    Diff { files: Vec<FilePreview> },
    Command { command: String },
    Note { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangePreview {
    pub body: PreviewBody,
    pub added: usize,
    pub removed: usize,
    /// A problem the client should show without hiding the diff.
    pub warning: Option<String>,
}

impl ChangePreview {
    pub fn note(message: impl Into<String>) -> Self {
        Self {
            body: PreviewBody::Note {
                message: message.into(),
            },
            added: 0,
            removed: 0,
            warning: None,
        }
    }

    pub fn command(command: impl Into<String>) -> Self {
        Self {
            body: PreviewBody::Command {
                command: command.into(),
            },
            added: 0,
            removed: 0,
            warning: None,
        }
    }

    pub fn diff(files: Vec<FilePreview>) -> Self {
        let added = files.iter().map(|file| file.added).sum();
        let removed = files.iter().map(|file| file.removed).sum();
        Self {
            body: PreviewBody::Diff { files },
            added,
            removed,
            warning: None,
        }
    }

    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warning = Some(warning.into());
        self
    }

    pub fn is_diff(&self) -> bool {
        matches!(&self.body, PreviewBody::Diff { files } if !files.is_empty())
    }

    pub fn files(&self) -> &[FilePreview] {
        match &self.body {
            PreviewBody::Diff { files } => files,
            _ => &[],
        }
    }
}

impl Toolbox {
    /// Build a preview for a mutating tool without changing the workspace.
    ///
    /// Reads use the same path resolution as execution. The preview can race
    /// with the eventual write; execution re-reads and re-validates.
    pub fn preview(&self, name: &str, input: &serde_json::Value) -> ChangePreview {
        match name {
            "write" => self.preview_write(input),
            "edit" => self.preview_edit(input),
            "bash" => match input_string(input, "command") {
                Ok(command) if !command.trim().is_empty() => ChangePreview::command(command),
                Ok(_) => ChangePreview::note("command must not be empty"),
                Err(error) => ChangePreview::note(format!("{error:#}")),
            },
            other => ChangePreview::note(format!("no preview for tool {other:?}")),
        }
    }

    fn preview_write(&self, input: &serde_json::Value) -> ChangePreview {
        let raw = match input_string(input, "path") {
            Ok(raw) => raw,
            Err(error) => return ChangePreview::note(format!("{error:#}")),
        };
        let path = match self.resolve_for_write(raw) {
            Ok(path) => path,
            Err(error) => return ChangePreview::note(format!("{error:#}")),
        };
        let content = match input_string(input, "content") {
            Ok(content) => content,
            Err(error) => return ChangePreview::note(format!("{error:#}")),
        };
        let display = self.display_path(&path);
        let mut warning = self.policy_warning();

        if path.exists() {
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    return ChangePreview::note(format!("cannot read {display}: {error}"));
                }
            };
            let old = match String::from_utf8(bytes) {
                Ok(old) => old,
                Err(_) => {
                    return ChangePreview::note(format!(
                        "{display}: existing file is not valid UTF-8; full diff unavailable"
                    ));
                }
            };
            if !self.is_observed(&path) {
                warning = Some(format!(
                    "read {display} before writing it; this call will be rejected"
                ));
            }
            let file = FilePreview::from_text(display, &old, content, false);
            return finish(ChangePreview::diff(vec![file]), warning);
        }

        let file = FilePreview::from_text(display, "", content, true);
        finish(ChangePreview::diff(vec![file]), warning)
    }

    fn preview_edit(&self, input: &serde_json::Value) -> ChangePreview {
        let raw = match input_string(input, "path") {
            Ok(raw) => raw,
            Err(error) => return ChangePreview::note(format!("{error:#}")),
        };
        let path = match self.resolve_existing(raw) {
            Ok(path) => path,
            Err(error) => return ChangePreview::note(format!("{error:#}")),
        };
        let display = self.display_path(&path);
        let mut warning = self.policy_warning();
        if !self.is_observed(&path) {
            warning = Some(format!(
                "read {display} before editing it; this call will be rejected"
            ));
        }

        let old = match input_string(input, "old") {
            Ok(old) => old,
            Err(error) => return ChangePreview::note(format!("{error:#}")),
        };
        let new = match input_string(input, "new") {
            Ok(new) => new,
            Err(error) => return ChangePreview::note(format!("{error:#}")),
        };
        if old.is_empty() {
            return finish(ChangePreview::note("old string must not be empty"), warning);
        }

        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                return ChangePreview::note(format!("cannot read {display}: {error}"));
            }
        };
        match text.match_indices(old).count() {
            0 => finish(
                ChangePreview::note(format!("old string not found in {display}")),
                warning,
            ),
            1 => {
                let updated = text.replacen(old, new, 1);
                let file = FilePreview::from_text(display, &text, &updated, false);
                finish(ChangePreview::diff(vec![file]), warning)
            }
            count => finish(
                ChangePreview::note(format!(
                    "old string is not unique in {display} ({count} matches); include more context"
                )),
                warning,
            ),
        }
    }

    fn policy_warning(&self) -> Option<String> {
        (self.policy.mode == crate::SandboxMode::ReadOnly)
            .then(|| "sandbox is read-only; this change will be rejected".to_string())
    }
}

fn finish(mut preview: ChangePreview, warning: Option<String>) -> ChangePreview {
    preview.warning = warning;
    preview
}

fn header_range(start: usize, end: usize) -> (usize, usize) {
    let len = end.saturating_sub(start);
    if len == 0 {
        (start, 0)
    } else {
        (start + 1, len)
    }
}

fn line_text(line: &str) -> String {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line).to_string()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::*;
    use crate::{NetworkMode, SandboxMode, SandboxPolicy, Toolbox};

    fn temp_workspace(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("niucode-preview-{tag}-{stamp}-{sequence}"));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create workspace");
        directory
    }

    fn toolbox(directory: &Path) -> Toolbox {
        Toolbox::with_policy(
            directory,
            SandboxPolicy::new(SandboxMode::FullAccess, NetworkMode::On),
        )
        .expect("toolbox")
    }

    #[test]
    fn new_file_preview_is_all_additions() {
        let file = FilePreview::from_text("a.txt", "", "one\ntwo\n", true);
        assert!(file.is_new);
        assert_eq!((file.added, file.removed), (2, 0));
        assert_eq!(file.hunks.len(), 1);
        let hunk = &file.hunks[0];
        assert_eq!((hunk.old_start, hunk.old_len), (0, 0));
        assert_eq!((hunk.new_start, hunk.new_len), (1, 2));
        assert_eq!(hunk.lines[0].new_line, Some(1));
        assert_eq!(hunk.lines[0].text, "one");
    }

    #[test]
    fn modified_file_preview_keeps_context_and_line_numbers() {
        let old = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n";
        let new = "a\nb\nc\nd\nE\nf\ng\nh\ni\nj\n";
        let file = FilePreview::from_text("a.txt", old, new, false);
        assert_eq!((file.added, file.removed), (1, 1));
        let hunk = &file.hunks[0];
        assert!(hunk.old_start >= 1);
        let removed = hunk
            .lines
            .iter()
            .find(|line| line.kind == LineKind::Remove)
            .expect("removed line");
        assert_eq!(removed.old_line, Some(5));
        assert_eq!(removed.new_line, None);
        let added = hunk
            .lines
            .iter()
            .find(|line| line.kind == LineKind::Add)
            .expect("added line");
        assert_eq!(added.old_line, None);
        assert_eq!(added.new_line, Some(5));
    }

    #[test]
    fn separated_changes_produce_multiple_hunks() {
        let old = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\n";
        let new = "a\nB\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nN\no\np\n";
        let file = FilePreview::from_text("a.txt", old, new, false);
        assert_eq!(file.hunks.len(), 2);
        assert_eq!((file.added, file.removed), (2, 2));
    }

    #[tokio::test]
    async fn write_preview_survives_execution_checks() {
        let directory = temp_workspace("write");
        fs::write(directory.join("a.txt"), "old\n").expect("fixture");
        let tools = toolbox(&directory);
        tools.execute("read", &json!({"path": "a.txt"})).await;

        let preview = tools.preview("write", &json!({"path": "a.txt", "content": "new\n"}));
        assert!(preview.is_diff());
        assert_eq!((preview.added, preview.removed), (1, 1));
        assert_eq!(preview.files()[0].path, "a.txt");
        assert!(preview.warning.is_none());
        assert_eq!(
            fs::read_to_string(directory.join("a.txt")).unwrap(),
            "old\n"
        );

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn write_preview_warns_when_file_was_not_read() {
        let directory = temp_workspace("unread");
        fs::write(directory.join("a.txt"), "old\n").expect("fixture");
        let tools = toolbox(&directory);

        let preview = tools.preview("write", &json!({"path": "a.txt", "content": "new\n"}));
        assert!(preview.is_diff());
        assert!(preview
            .warning
            .as_deref()
            .is_some_and(|warning| warning.contains("will be rejected")));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[tokio::test]
    async fn edit_preview_shows_the_resulting_diff() {
        let directory = temp_workspace("edit");
        fs::write(directory.join("a.txt"), "one\ntwo\nthree\n").expect("fixture");
        let tools = toolbox(&directory);
        tools.execute("read", &json!({"path": "a.txt"})).await;

        let preview = tools.preview(
            "edit",
            &json!({"path": "a.txt", "old": "two", "new": "TWO"}),
        );
        assert!(preview.is_diff());
        assert_eq!((preview.added, preview.removed), (1, 1));
        let lines = &preview.files()[0].hunks[0].lines;
        assert!(lines
            .iter()
            .any(|line| line.kind == LineKind::Remove && line.text == "two"));
        assert!(lines
            .iter()
            .any(|line| line.kind == LineKind::Add && line.text == "TWO"));
        assert_eq!(
            fs::read_to_string(directory.join("a.txt")).unwrap(),
            "one\ntwo\nthree\n"
        );

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn edit_preview_explains_non_unique_old_string() {
        let directory = temp_workspace("ambiguous");
        fs::write(directory.join("a.txt"), "x x\n").expect("fixture");
        let tools = toolbox(&directory);

        let preview = tools.preview("edit", &json!({"path": "a.txt", "old": "x", "new": "y"}));
        assert!(!preview.is_diff());
        assert!(matches!(
            preview.body,
            PreviewBody::Note { ref message } if message.contains("not unique")
        ));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn preview_rejects_paths_outside_workspace() {
        let directory = temp_workspace("escape");
        let tools = toolbox(&directory);
        let preview = tools.preview(
            "write",
            &json!({"path": "../outside.txt", "content": "bad"}),
        );
        assert!(!preview.is_diff());
        assert!(matches!(
            preview.body,
            PreviewBody::Note { ref message } if message.contains("escapes")
        ));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn binary_old_file_has_no_text_diff() {
        let directory = temp_workspace("binary");
        fs::write(directory.join("a.bin"), [0xff, 0xfe, 0x00]).expect("fixture");
        let tools = toolbox(&directory);

        let preview = tools.preview("write", &json!({"path": "a.bin", "content": "text"}));
        assert!(!preview.is_diff());
        assert!(matches!(
            preview.body,
            PreviewBody::Note { ref message } if message.contains("not valid UTF-8")
        ));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn bash_preview_echoes_the_command() {
        let directory = temp_workspace("bash");
        let tools = toolbox(&directory);
        let preview = tools.preview("bash", &json!({"command": "cargo test"}));
        assert!(matches!(
            preview.body,
            PreviewBody::Command { ref command } if command == "cargo test"
        ));

        fs::remove_dir_all(directory).expect("cleanup");
    }
}
