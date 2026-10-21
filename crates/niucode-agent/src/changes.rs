//! Bounded workspace evidence. Baselines are session events, never a side store.
use niucode_core::{Event, FileSnapshot, ProcessOutcome, WorkspaceSnapshot};
use niucode_tools::digest;
use similar::TextDiff;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;

const MAX_FILES: usize = 2000;
const MAX_FILE: u64 = 512 * 1024;
const MAX_BYTES: usize = 4 * 1024 * 1024;

/// The deadline covers enumeration and reads. Per-file metadata is checked
/// before/after reading; a live workspace snapshot is not a transaction.
pub(crate) async fn capture(root: &Path, session_path: &Path, text: bool) -> WorkspaceSnapshot {
    let started = Instant::now();
    let mut snapshot = WorkspaceSnapshot {
        complete: true,
        files: Vec::new(),
        notes: Vec::new(),
    };
    let root = match tokio::fs::canonicalize(root).await {
        Ok(root) => root,
        Err(error) => {
            snapshot.complete = false;
            snapshot
                .notes
                .push(format!("workspace unavailable: {error}"));
            return snapshot;
        }
    };
    let root = root.as_path();
    let artifact_directory = session_path
        .ancestors()
        .find(|p| p.extension().is_some_and(|ext| ext == "subagents"))
        .map(Path::to_path_buf)
        .unwrap_or_else(|| session_path.with_extension("subagents"));
    let parent_log = artifact_directory.with_extension("jsonl");
    let paths = match tokio::time::timeout(Duration::from_secs(1), git_paths(root)).await {
        Ok(Some(paths)) => paths,
        _ => {
            snapshot.complete = false;
            snapshot
                .notes
                .push("Git enumeration unavailable; bounded non-ignored file coverage only".into());
            let mut paths = Vec::new();
            let walker = ignore::WalkBuilder::new(root)
                .hidden(false)
                .require_git(false)
                .follow_links(false)
                .filter_entry(|e| {
                    !matches!(
                        e.file_name().to_str(),
                        Some(".git" | "target" | "node_modules" | ".niucode")
                    )
                })
                .build();
            for entry in walker {
                tokio::task::yield_now().await;
                if paths.len() >= MAX_FILES || started.elapsed() > Duration::from_secs(2) {
                    break;
                }
                if let Ok(e) = entry {
                    if e.file_type().is_some_and(|t| t.is_file() || t.is_symlink()) {
                        if let Ok(path) = e.path().strip_prefix(root) {
                            paths.push(path.to_path_buf());
                        }
                    }
                }
            }
            paths
        }
    };
    let mut bytes = 0;
    for path in paths {
        tokio::task::yield_now().await;
        if snapshot.files.len() >= MAX_FILES
            || bytes >= MAX_BYTES
            || started.elapsed() > Duration::from_secs(2)
        {
            snapshot.complete = false;
            snapshot
                .notes
                .push("workspace evidence budget reached (2000 files / 4 MiB / 2s)".into());
            break;
        }
        if path.is_absolute()
            || path
                .components()
                .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
        {
            snapshot.complete = false;
            continue;
        }
        let absolute = root.join(&path);
        if absolute == session_path
            || absolute == parent_log
            || absolute.starts_with(&artifact_directory)
        {
            continue;
        }
        let Some(name) = path.to_str() else {
            snapshot.complete = false;
            snapshot
                .notes
                .push("non-UTF-8 path omitted from review".into());
            continue;
        };
        let name = name.replace('\\', "/");
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(".env"))
        {
            snapshot.complete = false;
            snapshot.notes.push(format!(
                "{name}: environment file excluded from content capture"
            ));
            continue;
        }
        let metadata = match tokio::fs::symlink_metadata(&absolute).await {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                snapshot.complete = false;
                snapshot.notes.push(format!("{name}: {e}"));
                continue;
            }
        };
        if !metadata.is_file()
            || metadata.len() > MAX_FILE
            || bytes + metadata.len() as usize > MAX_BYTES
        {
            snapshot.complete = false;
            snapshot.files.push(FileSnapshot {
                path: name,
                digest: None,
                text: None,
                mode: None,
                note: Some("symlink/non-file or content budget; not fully covered".into()),
            });
            continue;
        }
        // Never follow a tracked symlink or symlinked parent outside the workspace.
        let canonical = match tokio::fs::canonicalize(&absolute).await {
            Ok(p) if p.starts_with(root) => p,
            _ => {
                snapshot.complete = false;
                snapshot
                    .notes
                    .push(format!("{name}: path resolution changed/outside workspace"));
                continue;
            }
        };
        let content =
            match tokio::time::timeout(Duration::from_millis(200), tokio::fs::read(&canonical))
                .await
            {
                Ok(Ok(content)) => content,
                _ => {
                    snapshot.complete = false;
                    snapshot.notes.push(format!("{name}: content unavailable"));
                    continue;
                }
            };
        bytes += content.len();
        let after = tokio::fs::metadata(&absolute).await.ok();
        let stable = after
            .as_ref()
            .is_some_and(|after| same_metadata(&metadata, after))
            && content.len() as u64 == metadata.len();
        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::MetadataExt;
            Some(metadata.mode())
        };
        #[cfg(not(unix))]
        let mode = None;
        let (hash, body, note) = if stable {
            let hash = Some(digest(&content));
            match String::from_utf8(content) {
                Ok(body) => (hash, text.then_some(body), None),
                Err(_) => (hash, None, Some("binary content: digest only".into())),
            }
        } else {
            snapshot.complete = false;
            (None, None, Some("file changed during capture".into()))
        };
        snapshot.files.push(FileSnapshot {
            path: name,
            digest: hash,
            text: body,
            mode,
            note,
        });
    }
    snapshot.files.sort_by(|a, b| a.path.cmp(&b.path));
    snapshot
}
fn same_metadata(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        a.dev() == b.dev()
            && a.ino() == b.ino()
            && a.len() == b.len()
            && a.mode() == b.mode()
            && a.mtime() == b.mtime()
            && a.mtime_nsec() == b.mtime_nsec()
            && a.ctime() == b.ctime()
            && a.ctime_nsec() == b.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        a.len() == b.len() && a.modified().ok() == b.modified().ok()
    }
}
async fn git_paths(root: &Path) -> Option<Vec<PathBuf>> {
    let mut command = tokio::process::Command::new("git");
    command
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = command.spawn().ok()?;
    let pipe = child.stdout.take()?;
    let mut bytes = Vec::new();
    pipe.take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .await
        .ok()?;
    if bytes.len() > 4 * 1024 * 1024 {
        return None;
    }
    if !child.wait().await.ok()?.success() {
        return None;
    }
    let paths: BTreeSet<_> = bytes
        .split(|&b| b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| {
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStringExt;
                PathBuf::from(std::ffi::OsString::from_vec(p.to_vec()))
            }
            #[cfg(not(unix))]
            {
                PathBuf::from(String::from_utf8_lossy(p).to_string())
            }
        })
        .collect();
    Some(paths.into_iter().collect())
}

pub(crate) fn fingerprint(snapshot: &WorkspaceSnapshot) -> Option<String> {
    if !snapshot.complete || snapshot.files.iter().any(|f| f.digest.is_none()) {
        return None;
    }
    let mut bytes = Vec::new();
    for file in &snapshot.files {
        bytes.extend_from_slice(file.path.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(file.digest.as_ref()?.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&file.mode.unwrap_or_default().to_le_bytes());
    }
    Some(digest(&bytes))
}

/// Review facts, independent of terminal formatting. Text diffs are artifacts.
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewedFile {
    pub path: String,
    pub attributed: bool,
    pub diff: Option<String>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CommandCheck {
    pub call_id: String,
    pub command: String,
    pub outcome: ProcessOutcome,
    pub fresh: bool,
}
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TaskReview {
    pub id: String,
    pub cwd: String,
    pub status: Option<String>,
    pub elapsed_ms: u64,
    pub changed: Vec<ReviewedFile>,
    pub unknown: Vec<String>,
    pub checks: Vec<CommandCheck>,
    pub conflicts: Vec<String>,
    pub tool_failures: usize,
    pub denied: usize,
    pub writes: usize,
}

/// Only verified bodies may be reused. The bounded index is disposable;
/// eviction sacrifices deduplication, never snapshot coverage.
pub(crate) fn recorded_bodies(events: &[Event]) -> BTreeMap<&str, &str> {
    let mut bodies = BTreeMap::new();
    for event in events {
        if let Event::TaskSnapshot { snapshot, .. } = event {
            for file in &snapshot.files {
                if let (Some(hash), Some(body)) = (&file.digest, &file.text) {
                    if digest(body.as_bytes()) == *hash {
                        bodies.insert(hash.as_str(), body.as_str());
                    }
                }
            }
        }
    }
    bodies
}
pub(crate) fn deduplicate(snapshot: &mut WorkspaceSnapshot, known: &mut BTreeSet<String>) {
    for file in &mut snapshot.files {
        if let (Some(hash), Some(_)) = (&file.digest, &file.text) {
            if known.contains(hash) {
                file.text = None;
            } else if known.len() < 32_768 {
                known.insert(hash.clone());
            }
        }
    }
}

/// Select one task without losing the earlier bodies its snapshots reference.
pub fn task_review_by_id(events: &[Event], id: &str) -> Option<TaskReview> {
    let start = events
        .iter()
        .position(|e| matches!(e, Event::TaskStarted { id: candidate, .. } if candidate == id))?;
    let end = events
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, e)| matches!(e, Event::TaskStarted { .. }))
        .map_or(events.len(), |(i, _)| i);
    project_review(&events[..end], start)
}
pub fn task_review(events: &[Event]) -> Option<TaskReview> {
    let start = events
        .iter()
        .rposition(|e| matches!(e, Event::TaskStarted { .. }))?;
    project_review(events, start)
}
fn project_review(all: &[Event], start: usize) -> Option<TaskReview> {
    let Event::TaskStarted { id, cwd, ts } = &all[start] else {
        return None;
    };
    let events = &all[start..];
    let mut review = TaskReview {
        id: id.clone(),
        cwd: cwd.clone(),
        ..Default::default()
    };
    for event in events {
        match event {
            Event::TaskFinished {
                id: task,
                status,
                ts: end,
            } if task == id => {
                review.status = Some(status.clone());
                review.elapsed_ms = end.saturating_sub(*ts);
            }
            Event::ToolResult { ok: false, .. } => review.tool_failures += 1,
            Event::Approval { allowed: false, .. } => review.denied += 1,
            Event::ToolEvidence { evidence, .. } if evidence.change.is_some() => review.writes += 1,
            _ => {}
        }
    }
    let snapshot = |phase: &str| {
        all.iter()
            .enumerate()
            .skip(start)
            .rev()
            .find_map(|(i, event)| match event {
                Event::TaskSnapshot {
                    task_id,
                    phase: p,
                    snapshot,
                    ..
                } if task_id == id && p == phase => Some((i, snapshot)),
                _ => None,
            })
    };
    let before = snapshot("before");
    let after = snapshot("after");
    if let (Some((bi, before)), Some((ai, after))) = (before, after) {
        // A body appearing after a snapshot must not repair missing historical evidence.
        let mut before_bodies = None;
        let mut after_bodies = None;
        let before_files: BTreeMap<_, _> =
            before.files.iter().map(|f| (f.path.as_str(), f)).collect();
        let after_files: BTreeMap<_, _> =
            after.files.iter().map(|f| (f.path.as_str(), f)).collect();
        let names: BTreeSet<_> = before_files
            .keys()
            .chain(after_files.keys())
            .copied()
            .collect();
        for path in names {
            let old = before_files.get(path).copied();
            let new = after_files.get(path).copied();
            let content_known = match (old, new) {
                (Some(a), Some(b)) => a.digest.is_some() && b.digest.is_some(),
                (None, Some(b)) => before.complete && b.digest.is_some(),
                (Some(a), None) => after.complete && a.digest.is_some(),
                _ => false,
            };
            let changed = match (old, new) {
                (Some(a), Some(b)) => {
                    (content_known && a.digest != b.digest)
                        || (a.mode.is_some() && b.mode.is_some() && a.mode != b.mode)
                }
                (None, Some(_)) => before.complete,
                (Some(_), None) => after.complete,
                _ => false,
            };
            if !content_known {
                let note = new
                    .and_then(|f| f.note.as_deref())
                    .or_else(|| old.and_then(|f| f.note.as_deref()))
                    .unwrap_or("content or directory coverage unavailable");
                review.unknown.push(format!("{path}: {note}"));
            }
            if !changed {
                continue;
            }
            let mut expected = old.and_then(|f| f.digest.clone());
            let mut attributed = false;
            let mut consistent = content_known;
            for event in events {
                if let Event::ToolEvidence { evidence, .. } = event {
                    if let Some(change) = evidence.change.as_ref().filter(|c| c.path == path) {
                        consistent &= change.before_digest == expected;
                        expected = Some(change.after_digest.clone());
                        attributed = true;
                    }
                }
            }
            attributed &= consistent
                && expected == new.and_then(|f| f.digest.clone())
                && (old.is_none() || old.map(|f| f.mode) == new.map(|f| f.mode));
            let body = |file: Option<&FileSnapshot>,
                        complete: bool,
                        bodies: &BTreeMap<&str, &str>|
             -> Option<String> {
                match file {
                    Some(file) => file
                        .digest
                        .as_deref()
                        .and_then(|hash| bodies.get(hash))
                        .map(|s| (*s).to_string()),
                    None if complete => Some(String::new()),
                    None => None,
                }
            };
            let diff = body(
                old,
                before.complete,
                before_bodies.get_or_insert_with(|| recorded_bodies(&all[..=bi])),
            )
            .zip(body(
                new,
                after.complete,
                after_bodies.get_or_insert_with(|| recorded_bodies(&all[..=ai])),
            ))
            .map(|(a, b)| {
                if a == b {
                    "content unchanged; metadata changed\n".into()
                } else {
                    TextDiff::from_lines(&a, &b)
                        .unified_diff()
                        .context_radius(3)
                        .header("task start", "task end")
                        .to_string()
                }
            });
            review.changed.push(ReviewedFile {
                path: path.into(),
                attributed,
                diff,
            });
        }
        review
            .unknown
            .extend(before.notes.iter().chain(&after.notes).cloned());
        if (!before.complete || !after.complete) && review.unknown.is_empty() {
            review.unknown.push("workspace coverage incomplete".into());
        }
    } else {
        review
            .unknown
            .push("workspace review incomplete; snapshot unavailable".into());
    }
    let final_fingerprint = after.and_then(|(_, s)| fingerprint(s));
    for (index, event) in events.iter().enumerate() {
        if let Event::ToolEvidence {
            call_id, evidence, ..
        } = event
        {
            if let Some(process) = &evidence.process {
                let command = events[..index]
                    .iter()
                    .rev()
                    .find_map(|event| match event {
                        Event::AssistantMessage { tool_calls, .. } => tool_calls
                            .iter()
                            .find(|c| &c.id == call_id)
                            .and_then(|c| c.input.get("command"))
                            .and_then(|v| v.as_str()),
                        _ => None,
                    })
                    .unwrap_or("(command not recorded)");
                let checked = events[index + 1..].iter().find_map(|event| match event {
                    Event::CheckSnapshot {
                        call_id: candidate,
                        digest,
                        ..
                    } if candidate == call_id => digest.as_ref(),
                    _ => None,
                });
                review.checks.push(CommandCheck {
                    call_id: call_id.clone(),
                    command: command.into(),
                    outcome: process.clone(),
                    fresh: checked.is_some() && checked == final_fingerprint.as_ref(),
                });
            }
            if let Some(conflict) = &evidence.conflict {
                review.conflicts.push(format!("{call_id}: {conflict}"));
            }
        }
    }
    review.unknown.sort();
    review.unknown.dedup();
    Some(review)
}

#[cfg(test)]
mod tests {
    use super::*;
    use niucode_core::{ExecutionEvidence, FileChange};
    use std::fs;

    fn git(root: &Path, args: &[&str]) -> Vec<u8> {
        let result = std::process::Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        result.stdout
    }
    fn event(snapshot: WorkspaceSnapshot, phase: &str) -> Event {
        Event::TaskSnapshot {
            task_id: "task".into(),
            phase: phase.into(),
            snapshot,
            ts: 0,
        }
    }

    #[tokio::test]
    async fn review_uses_dirty_baseline_preserves_index_and_marks_later_changes_stale() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        fs::write(root.join("tracked"), "staged\n").unwrap();
        git(root, &["add", "tracked"]);
        fs::write(root.join("tracked"), "dirty baseline\n").unwrap();
        fs::write(root.join("untracked"), "keep me\n").unwrap();
        let index_before = fs::read(root.join(".git/index")).unwrap();
        let before = capture(root, &root.join("session.jsonl"), true).await;
        assert!(before.complete, "{:?}", before.notes);
        fs::write(root.join("tracked"), "agent edit\n").unwrap();
        let checked = capture(root, &root.join("session.jsonl"), false).await;
        fs::write(root.join("untracked"), "external edit\n").unwrap();
        let after = capture(root, &root.join("session.jsonl"), true).await;
        let events = vec![
            Event::TaskStarted {
                id: "task".into(),
                cwd: root.display().to_string(),
                ts: 0,
            },
            event(before, "before"),
            Event::ToolEvidence {
                call_id: "edit".into(),
                evidence: ExecutionEvidence {
                    change: Some(FileChange {
                        path: "tracked".into(),
                        before_digest: Some(digest(b"dirty baseline\n")),
                        after_digest: digest(b"agent edit\n"),
                    }),
                    ..Default::default()
                },
                ts: 0,
            },
            Event::ToolEvidence {
                call_id: "check".into(),
                evidence: ExecutionEvidence {
                    process: Some(ProcessOutcome::Exited { code: 0 }),
                    ..Default::default()
                },
                ts: 0,
            },
            Event::CheckSnapshot {
                call_id: "check".into(),
                digest: fingerprint(&checked),
                ts: 0,
            },
            event(after, "after"),
        ];
        let review = task_review(&events).unwrap();
        let tracked = review.changed.iter().find(|f| f.path == "tracked").unwrap();
        assert!(tracked.diff.as_ref().unwrap().contains("-dirty baseline"));
        assert!(!tracked.diff.as_ref().unwrap().contains("-staged"));
        assert!(tracked.attributed);
        assert!(
            !review
                .changed
                .iter()
                .find(|f| f.path == "untracked")
                .unwrap()
                .attributed
        );
        assert!(!review.checks[0].fresh);
        assert_eq!(fs::read(root.join(".git/index")).unwrap(), index_before);
        let json = events
            .iter()
            .map(|e| serde_json::to_string(e).unwrap())
            .collect::<Vec<_>>();
        let rebuilt: Vec<Event> = json
            .iter()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert_eq!(task_review(&rebuilt).unwrap(), review);
    }

    #[tokio::test]
    async fn binary_large_and_non_git_coverage_are_explicit() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("binary"), [0xff, 0, 0xfe]).unwrap();
        fs::write(root.join("large"), vec![b'a'; MAX_FILE as usize + 1]).unwrap();
        let snapshot = capture(root, &root.join("session"), true).await;
        assert!(!snapshot.complete);
        assert!(fingerprint(&snapshot).is_none());
        let binary = snapshot.files.iter().find(|f| f.path == "binary").unwrap();
        assert!(binary.digest.is_some() && binary.text.is_none());
        assert!(binary.note.as_ref().unwrap().contains("binary"));
        let large = snapshot.files.iter().find(|f| f.path == "large").unwrap();
        assert!(large.digest.is_none());
        assert!(large.note.as_ref().unwrap().contains("budget"));
    }

    #[test]
    fn interrupted_task_never_implies_success_or_automatic_replay() {
        let review = task_review(&[Event::TaskStarted {
            id: "task".into(),
            cwd: "/workspace".into(),
            ts: 0,
        }])
        .unwrap();
        assert!(review.unknown[0].contains("incomplete"));
        assert_eq!(review.status, None);
    }
    #[test]
    fn identical_partial_snapshots_are_unknown_not_changes() {
        let files = (0..394)
            .map(|i| FileSnapshot {
                path: format!("file-{i:03}"),
                digest: (i < 264).then(|| digest(b"same")),
                text: (i < 264).then(|| "same".into()),
                mode: Some(0o100644),
                note: (i >= 264).then(|| "content budget".into()),
            })
            .collect();
        let before = WorkspaceSnapshot {
            files,
            complete: false,
            notes: vec![],
        };
        let mut events = vec![
            Event::TaskStarted {
                id: "task".into(),
                cwd: "/work".into(),
                ts: 0,
            },
            event(before.clone(), "before"),
            event(before, "after"),
        ];
        let review = task_review(&events).unwrap();
        assert!(review.changed.is_empty());
        assert_eq!(review.unknown.len(), 130);
        assert!(review.checks.is_empty());
        // An absent entry under incomplete coverage does not prove a deletion.
        if let Event::TaskSnapshot { snapshot, .. } = &mut events[2] {
            snapshot.files.remove(0);
        }
        assert!(task_review(&events).unwrap().changed.is_empty());
        // A proven mode change remains visible even when content is unknown.
        if let Event::TaskSnapshot { snapshot, .. } = &mut events[2] {
            snapshot.files.last_mut().unwrap().mode = Some(0o100755);
        }
        let review = task_review(&events).unwrap();
        assert_eq!(review.changed.len(), 1);
        assert!(!review.changed[0].attributed);
    }

    #[test]
    fn deduplicated_bodies_replay_from_prefix_not_future_events() {
        let file = |text: &str| FileSnapshot {
            path: "file".into(),
            digest: Some(digest(text.as_bytes())),
            text: Some(text.into()),
            mode: Some(0o100644),
            note: None,
        };
        let snapshot = |text: &str| WorkspaceSnapshot {
            files: vec![file(text)],
            complete: true,
            notes: vec![],
        };
        let mut known = BTreeSet::new();
        let mut initial = snapshot("old\n");
        deduplicate(&mut initial, &mut known);
        let mut before = snapshot("old\n");
        deduplicate(&mut before, &mut known);
        assert!(before.files[0].text.is_none());
        let mut events = vec![
            event(initial, "before"),
            Event::TaskStarted {
                id: "task".into(),
                cwd: "/work".into(),
                ts: 0,
            },
            event(before, "before"),
            event(snapshot("new\n"), "after"),
        ];
        let review = task_review_by_id(&events, "task").unwrap();
        assert!(review.changed[0]
            .diff
            .as_ref()
            .unwrap()
            .contains("-old\n+new"));
        let serialized = serde_json::to_vec(&events).unwrap();
        let restored: Vec<Event> = serde_json::from_slice(&serialized).unwrap();
        assert_eq!(task_review_by_id(&restored, "task").unwrap(), review);
        events.remove(0); // missing prefix cannot be reconstructed from future data
        events.push(event(snapshot("old\n"), "unrelated"));
        assert!(task_review_by_id(&events, "task").unwrap().changed[0]
            .diff
            .is_none());
    }

    #[test]
    fn complete_coverage_proves_additions_and_deletions() {
        let file = FileSnapshot {
            path: "created".into(),
            digest: None,
            text: None,
            mode: None,
            note: Some("large file".into()),
        };
        let empty = WorkspaceSnapshot {
            complete: true,
            ..Default::default()
        };
        let partial = WorkspaceSnapshot {
            files: vec![file],
            complete: false,
            notes: vec![],
        };
        let task = Event::TaskStarted {
            id: "task".into(),
            cwd: "/work".into(),
            ts: 0,
        };
        let added = task_review(&[
            task.clone(),
            event(empty.clone(), "before"),
            event(partial.clone(), "after"),
        ])
        .unwrap();
        assert_eq!(added.changed.len(), 1);
        let deleted =
            task_review(&[task, event(partial, "before"), event(empty, "after")]).unwrap();
        assert_eq!(deleted.changed.len(), 1);
        assert!(!deleted.changed[0].attributed);
    }
}
