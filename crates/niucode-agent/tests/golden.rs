use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use niucode_agent::{run, AgentConfig};
use niucode_core::session::load;
use niucode_core::{Event, SessionLog};
use niucode_provider::ReplayProvider;

fn temp_workspace(tag: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("niucode-golden-{tag}-{stamp}-{sequence}"));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create workspace");
    directory
}

#[tokio::test]
async fn golden_read_transcript_is_offline_and_stable() {
    let directory = temp_workspace("read");
    fs::write(directory.join("a.txt"), "golden content\n").expect("fixture");

    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/golden-read.json");
    let provider = ReplayProvider::load(&fixture).expect("load fixture");
    let mut session = SessionLog::create(directory.as_path()).expect("session");
    let config = AgentConfig::new("replay-model", directory.clone());

    let outcome = run(
        &provider,
        &mut session,
        &config,
        "read a.txt",
        &mut |_| true,
        &mut |_| {},
    )
    .await
    .expect("run golden transcript");

    assert_eq!(outcome.final_text, "read it");
    assert_eq!(outcome.turns, 2);
    assert_eq!(outcome.usage.input_tokens, 10);
    assert_eq!(outcome.usage.output_tokens, 2);

    let loaded = load(session.path()).expect("load session");
    let kinds = loaded.events.iter().map(Event::kind).collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            "user_message",
            "assistant_message",
            "turn_end",
            "tool_result",
            "assistant_message",
            "turn_end",
            "session_end"
        ]
    );

    fs::remove_dir_all(directory).expect("cleanup");
}
