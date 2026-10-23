use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use niucode_agent::{project_history, run, run_with_history, AgentConfig, ApprovalMode};
use niucode_core::{load, Event, SessionLog};
use niucode_provider::{ScriptedProvider, StreamEvent};
use niucode_tools::{NetworkMode, SandboxMode, SandboxPolicy};

fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("niucode-inst-{tag}-{stamp}-{sequence}"));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create temp dir");
    directory
}

#[tokio::test]
async fn agents_md_changes_apply_to_the_next_turn() {
    let root = temp_dir("agents");
    let child = root.join("crates/app");
    fs::create_dir_all(&child).expect("child dir");
    fs::write(root.join("AGENTS.md"), "root rule: keep it small\n").expect("root instructions");
    let child_instructions = child.join("AGENTS.md");
    fs::write(&child_instructions, "child rule: use tabs\n").expect("child instructions");

    let provider = ScriptedProvider::from_turns(vec![
        vec![
            StreamEvent::TextDelta("first".into()),
            StreamEvent::Done {
                finish_reason: Some("stop".into()),
            },
        ],
        vec![
            StreamEvent::TextDelta("second".into()),
            StreamEvent::Done {
                finish_reason: Some("stop".into()),
            },
        ],
    ]);
    let mut session = SessionLog::create(&root).expect("session");
    session
        .append(&Event::SessionStart {
            id: session.id().to_string(),
            cwd: child.display().to_string(),
            model: "test-model".into(),
            provider: "deepseek".into(),
            ts: 1,
        })
        .expect("session start");

    let mut config = AgentConfig::new("test-model", child.clone());
    config.sandbox = SandboxPolicy::new(SandboxMode::FullAccess, NetworkMode::On);
    config.approval = ApprovalMode::Never;

    run(
        &provider,
        &mut session,
        &config,
        "first task",
        &mut |_| true,
        &mut |_| {},
    )
    .await
    .expect("first run");

    fs::write(&child_instructions, "child rule: use spaces\n").expect("rewrite instructions");
    let history = project_history(&load(session.path()).expect("load").events).expect("history");
    run_with_history(
        &provider,
        &mut session,
        &config,
        "second task",
        history,
        &mut |_| true,
        &mut |_| {},
    )
    .await
    .expect("second run");

    let requests = provider.requests();
    let first_system = requests[0].messages[0].text().expect("system");
    assert!(first_system.contains("root rule: keep it small"));
    assert!(first_system.contains("child rule: use tabs"));
    let child_position = first_system.find("child rule").expect("child");
    let root_position = first_system.find("root rule").expect("root");
    assert!(child_position < root_position, "nearest instructions first");

    let second_system = requests[1].messages[0].text().expect("system");
    assert!(second_system.contains("child rule: use spaces"));
    assert!(!second_system.contains("child rule: use tabs"));

    let loaded = load(session.path()).expect("load");
    let instructions = loaded
        .events
        .iter()
        .filter_map(|event| match event {
            Event::Instructions {
                sources, digest, ..
            } => Some((sources.clone(), digest.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(instructions.len(), 2);
    assert_ne!(instructions[0].1, instructions[1].1);
    assert!(instructions[0].0.len() >= 2);
    assert!(instructions[0].0[0].ends_with("crates/app/AGENTS.md"));

    fs::remove_dir_all(root).expect("cleanup");
}
