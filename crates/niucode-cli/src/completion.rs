//! Completion data and cancellable workspace discovery; independent of drawing.
use crate::slash::CommandRegistry;
use ignore::WalkBuilder;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct Candidate {
    pub range: Range<usize>,
    pub insert: String,
    pub name: String,
    pub description: String,
    pub action: CompletionAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionAction {
    Insert,
    OpenPicker,
    Apply,
}

pub struct Completion {
    registry: CommandRegistry,
    pub models: Vec<String>,
    pub current_model: String,
    pub permissions: Vec<(String, String)>,
    pub skills: Vec<String>,
    files: Vec<String>,
    scan: Option<FileScan>,
    pub notice: Option<String>,
}

impl Completion {
    pub fn new(registry: CommandRegistry) -> Self {
        Self {
            registry,
            models: Vec::new(),
            current_model: String::new(),
            permissions: Vec::new(),
            skills: Vec::new(),
            files: Vec::new(),
            scan: None,
            notice: None,
        }
    }
    pub fn set_cwd(&mut self, cwd: PathBuf) {
        self.scan = Some(FileScan::start(cwd));
        self.files.clear();
        self.notice = None;
    }
    pub fn refresh(&mut self) {
        let result = self
            .scan
            .as_ref()
            .and_then(|scan| scan.receiver.try_recv().ok());
        if let Some((files, notice)) = result {
            self.files = files;
            self.notice = notice;
            self.scan = None;
        }
    }
    pub fn candidates(&self, text: &str, cursor: usize) -> Vec<Candidate> {
        if !text.is_char_boundary(cursor) {
            return Vec::new();
        }
        let before = &text[..cursor];
        let start = before
            .rfind(char::is_whitespace)
            .map_or(0, |i| i + before[i..].chars().next().unwrap().len_utf8());
        let end = text[cursor..]
            .find(char::is_whitespace)
            .map_or(text.len(), |i| cursor + i);
        let token = &text[start..cursor];
        let suffix = if text[end..].starts_with(char::is_whitespace) {
            ""
        } else {
            " "
        };
        let item = |name: String, description: String| Candidate {
            range: start..end,
            insert: format!("{name}{suffix}"),
            name,
            description,
            action: CompletionAction::Insert,
        };
        if let Some(prefix) = token.strip_prefix('$') {
            return self
                .skills
                .iter()
                .filter(|s| s.starts_with(prefix))
                .map(|s| item(format!("${s}"), "activate skill".into()))
                .collect();
        }
        if let Some(prefix) = token.strip_prefix('@') {
            return self
                .files
                .iter()
                .filter(|s| s.starts_with(prefix))
                .take(100)
                .map(|s| item(format!("@{s}"), "workspace file".into()))
                .collect();
        }
        if before == "/model" || before == "/permissions" {
            return self
                .arguments(before, "")
                .into_iter()
                .map(|(name, description)| Candidate {
                    range: 0..end,
                    insert: format!("{before} {name}{suffix}"),
                    name,
                    description,
                    action: CompletionAction::Apply,
                })
                .collect();
        }
        if start > 0 {
            let command = text[..start].trim_end();
            return self
                .arguments(command, token)
                .into_iter()
                .map(|(name, description)| Candidate {
                    action: CompletionAction::Apply,
                    ..item(name, description)
                })
                .collect();
        }
        if !token.starts_with('/') {
            return Vec::new();
        }
        self.registry
            .candidates(token)
            .into_iter()
            .map(|spec| Candidate {
                action: if matches!(spec.name.as_str(), "/model" | "/permissions") {
                    CompletionAction::OpenPicker
                } else {
                    CompletionAction::Insert
                },
                ..item(spec.name.clone(), spec.description.clone())
            })
            .collect()
    }
    fn arguments(&self, command: &str, prefix: &str) -> Vec<(String, String)> {
        match command {
            "/model" => self
                .models
                .iter()
                .filter(|s| s.starts_with(prefix))
                .map(|s| {
                    (
                        s.clone(),
                        if s == &self.current_model {
                            "current model"
                        } else {
                            "switch model"
                        }
                        .into(),
                    )
                })
                .collect(),
            "/permissions" => self
                .permissions
                .iter()
                .filter(|(s, _)| s.starts_with(prefix))
                .cloned()
                .collect(),
            _ => Vec::new(),
        }
    }
}

type ScanResult = (Vec<String>, Option<String>);
struct FileScan {
    cancel: Arc<AtomicBool>,
    receiver: mpsc::Receiver<ScanResult>,
    thread: Option<JoinHandle<()>>,
}
impl FileScan {
    fn start(root: PathBuf) -> Self {
        let (sender, receiver) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let cancelled = cancel.clone();
        let thread = std::thread::spawn(move || {
            let (files, notice) = collect_files(&root, &cancelled);
            let _ = sender.try_send((files, notice));
        });
        Self {
            cancel,
            receiver,
            thread: Some(thread),
        }
    }
}
impl Drop for FileScan {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn collect_files(root: &std::path::Path, cancelled: &AtomicBool) -> ScanResult {
    let mut files = Vec::new();
    let mut notice = None;
    let started = Instant::now();
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .follow_links(false)
        .filter_entry(|e| {
            e.depth() == 0 || {
                let name = e.file_name().to_string_lossy();
                !matches!(
                    name.as_ref(),
                    ".git" | "target" | "node_modules" | ".niucode"
                ) && !name.starts_with(".env")
            }
        })
        .build();
    for entry in walker {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        if files.len() >= 10_000 || started.elapsed() >= Duration::from_secs(2) {
            notice = Some("file completion index is partial (scan budget)".into());
            break;
        }
        match entry {
            Ok(e) if e.file_type().is_some_and(|t| t.is_file()) => {
                if let Ok(path) = e.path().strip_prefix(root) {
                    let path = path.to_string_lossy().replace('\\', "/");
                    if !path.chars().any(char::is_control) {
                        files.push(path);
                    }
                }
            }
            Err(_) => notice = Some("some workspace paths could not be indexed".into()),
            _ => {}
        }
    }
    files.sort();
    (files, notice)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_replaces_only_cursor_token() {
        let mut c = Completion::new(CommandRegistry::builtin());
        c.skills = vec!["rust".into()];
        let items = c.candidates("use $ru please", 7);
        assert_eq!(items[0].range, 4..7);
        assert_eq!(items[0].insert, "$rust");
        c.models = vec!["openai/a".into(), "openai/b".into()];
        c.current_model = "openai/a".into();
        assert_eq!(c.candidates("/model", 6)[0].insert, "/model openai/a ");
        assert_eq!(c.candidates("/model openai/b", 15).len(), 1);
    }
    #[test]
    fn discovery_respects_nested_ignore_and_cancellation() {
        let root =
            std::env::temp_dir().join(format!("niucode-completion-{}", niucode_core::now_ms()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/.gitignore"), "*.tmp\n!keep.tmp\n").unwrap();
        for name in ["src/hide.tmp", "src/keep.tmp", "src/a.rs", ".env"] {
            std::fs::write(root.join(name), "").unwrap();
        }
        let (files, _) = collect_files(&root, &AtomicBool::new(false));
        assert!(files.contains(&"src/keep.tmp".into()));
        assert!(files.contains(&"src/a.rs".into()));
        assert!(!files.contains(&"src/hide.tmp".into()));
        assert!(!files.contains(&".env".into()));
        assert!(collect_files(&root, &AtomicBool::new(true)).0.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
