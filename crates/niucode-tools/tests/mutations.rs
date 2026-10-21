use niucode_core::ProcessOutcome;
use niucode_tools::{NetworkMode, SandboxMode, SandboxPolicy, Toolbox};
use serde_json::json;
use std::fs;

fn toolbox(path: &std::path::Path) -> Toolbox {
    Toolbox::with_policy(
        path,
        SandboxPolicy::new(SandboxMode::FullAccess, NetworkMode::On),
    )
    .unwrap()
}

#[cfg(unix)]
#[tokio::test]
async fn read_roots_resolve_aliases_without_allowing_writes_or_retargeting() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    let root = dir.path().join("skills");
    let outside = dir.path().join("outside");
    let alias = dir.path().join("alias");
    for path in [&work, &root, &outside] {
        fs::create_dir(path).unwrap();
    }
    fs::write(root.join("SKILL.md"), "skill").unwrap();
    fs::write(outside.join("SKILL.md"), "outside").unwrap();
    symlink(&root, &alias).unwrap();
    let tools =
        Toolbox::with_policy_and_roots(&work, SandboxPolicy::default(), false, vec![alias.clone()])
            .unwrap();
    let input = json!({"path":alias.join("SKILL.md")});
    let read = tools.execute("read", &input).await;
    assert!(read.ok, "{}", read.output);
    assert!(
        !tools
            .execute(
                "write",
                &json!({"path":root.join("SKILL.md"),"content":"changed"})
            )
            .await
            .ok
    );
    fs::remove_file(&alias).unwrap();
    symlink(&outside, &alias).unwrap();
    assert!(!tools.execute("read", &input).await.ok);
    assert!(
        tools
            .execute("read", &json!({"path":root.join("SKILL.md")}))
            .await
            .ok
    );
    assert_eq!(fs::read_to_string(root.join("SKILL.md")).unwrap(), "skill");
}

#[tokio::test]
async fn equal_length_external_edit_conflicts_and_read_again_recovers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.txt");
    fs::write(&path, "one\n").unwrap();
    let tools = toolbox(dir.path());
    assert!(tools.execute("read", &json!({"path":"a.txt"})).await.ok);
    let mtime = fs::metadata(&path).unwrap().modified().unwrap();
    fs::write(&path, "two\n").unwrap();
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    let input = json!({"path":"a.txt","content":"mine\n"});
    let result = tools.execute_recorded("write", &input).await;
    assert!(!result.result.ok);
    assert!(result.evidence.conflict.is_some());
    assert_eq!(fs::read_to_string(&path).unwrap(), "two\n");
    assert!(tools.execute("read", &json!({"path":"a.txt"})).await.ok);
    let result = tools.execute_recorded("write", &input).await;
    assert!(result.result.ok);
    let change = result.evidence.change.unwrap();
    assert_ne!(change.before_digest.unwrap(), change.after_digest);
}

#[tokio::test]
async fn approval_is_bound_to_prepared_contents_even_after_reread() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a"), "old").unwrap();
    let tools = toolbox(dir.path());
    tools.execute("read", &json!({"path":"a"})).await;
    let prepared = tools
        .prepare_mutation("write", &json!({"path":"a","content":"new"}))
        .unwrap();
    assert!(prepared.preview().is_diff());
    fs::write(dir.path().join("a"), "external").unwrap();
    tools.execute("read", &json!({"path":"a"})).await;
    let result = tools.execute_prepared(prepared);
    assert!(result.evidence.conflict.is_some());
    assert_eq!(
        fs::read_to_string(dir.path().join("a")).unwrap(),
        "external"
    );
}

#[tokio::test]
async fn partial_reads_allow_only_observed_lines_and_new_files_never_clobber() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a"), "first\nsecond\nthird\n").unwrap();
    let tools = toolbox(dir.path());
    tools
        .execute("read", &json!({"path":"a","offset":2,"limit":1}))
        .await;
    assert!(
        !tools
            .execute("write", &json!({"path":"a","content":"replaced"}))
            .await
            .ok
    );
    assert!(
        !tools
            .execute("edit", &json!({"path":"a","old":"first","new":"FIRST"}))
            .await
            .ok
    );
    assert!(
        tools
            .execute("edit", &json!({"path":"a","old":"second","new":"SECOND"}))
            .await
            .ok
    );
    assert!(
        !tools
            .execute("write", &json!({"path":"a","content":"still unseen"}))
            .await
            .ok
    );
    let prepared = tools
        .prepare_mutation("write", &json!({"path":"new","content":"mine"}))
        .unwrap();
    fs::write(dir.path().join("new"), "theirs").unwrap();
    let result = tools.execute_prepared(prepared);
    assert!(result.evidence.conflict.is_some());
    assert_eq!(
        fs::read_to_string(dir.path().join("new")).unwrap(),
        "theirs"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn replacement_and_symlink_retarget_conflict_and_executable_mode_survives() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a");
    fs::write(&path, "same").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(dir.path().join("b"), "same").unwrap();
    symlink("a", dir.path().join("alias")).unwrap();
    let tools = toolbox(dir.path());
    tools.execute("read", &json!({"path":"alias"})).await;
    fs::remove_file(dir.path().join("alias")).unwrap();
    symlink("b", dir.path().join("alias")).unwrap();
    let result = tools
        .execute_recorded("write", &json!({"path":"alias","content":"new"}))
        .await;
    assert!(result.evidence.conflict.is_some());
    fs::write(dir.path().join("replacement"), "same").unwrap();
    fs::rename(dir.path().join("replacement"), &path).unwrap();
    let result = tools
        .execute_recorded("write", &json!({"path":"a","content":"new"}))
        .await;
    assert!(result.evidence.conflict.is_some());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    tools.execute("read", &json!({"path":"a"})).await;
    assert!(
        tools
            .execute("write", &json!({"path":"a","content":"new"}))
            .await
            .ok
    );
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3);
}

#[tokio::test]
async fn process_exit_is_evidence_separate_from_invocation_success() {
    let dir = tempfile::tempdir().unwrap();
    let tools = toolbox(dir.path());
    for code in [0, 7] {
        let result = tools
            .execute_recorded("bash", &json!({"command":format!("exit {code}")}))
            .await;
        assert!(result.result.ok);
        assert_eq!(
            result.evidence.process,
            Some(ProcessOutcome::Exited { code })
        );
    }
    #[cfg(unix)]
    {
        let result = tools
            .execute_recorded("bash", &json!({"command":"kill -TERM $$"}))
            .await;
        assert!(matches!(
            result.evidence.process,
            Some(ProcessOutcome::Signaled { signal: Some(15) })
        ));
    }
    let result = tools
        .execute_recorded("bash", &json!({"command":"sleep 20", "timeout_secs":1}))
        .await;
    assert!(!result.result.ok);
    assert_eq!(result.evidence.process, Some(ProcessOutcome::TimedOut));
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn dropping_a_running_shell_stops_the_parent_and_descendants() {
    use std::time::Duration;
    let dir = tempfile::tempdir().unwrap();
    let tools = toolbox(dir.path());
    let input = json!({"command":"sleep 60 & child=$!; printf '%s %s' $$ $child > ready; wait"});
    let effect = tools.execute_recorded("bash", &input);
    let ready = async {
        loop {
            if let Ok(text) = fs::read_to_string(dir.path().join("ready")) {
                let pids: Vec<u32> = text
                    .split_whitespace()
                    .filter_map(|p| p.parse().ok())
                    .collect();
                if pids.len() == 2 {
                    return pids;
                }
            }
            tokio::task::yield_now().await;
        }
    };
    let pids = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            _ = effect => panic!("shell finished before cancellation"),
            pids = ready => pids,
        }
    })
    .await
    .unwrap(); // The losing tool future and its process-group guard drop here.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let running = pids.iter().any(|pid| {
                fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
                    !matches!(
                        stat.rsplit_once(')')
                            .and_then(|(_, tail)| tail.split_whitespace().next()),
                        Some("Z" | "X")
                    )
                })
            });
            if !running {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancelled shell descendants still running");
}
