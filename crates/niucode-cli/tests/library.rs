use std::path::Path;

use niucode::{AgentEvent, ExecOptions, RunOutcome, TuiOptions, TurnHooks};

#[derive(Default)]
struct Recorder {
    saw_prompt: bool,
    events: usize,
    turns: usize,
}

impl TurnHooks for Recorder {
    fn augment_system_prompt(&mut self, prompt: &mut String) {
        self.saw_prompt = !prompt.is_empty();
        prompt.push_str("\n<!-- hook marker -->");
    }

    fn on_turn_end(&mut self, _outcome: &RunOutcome) {
        self.turns += 1;
    }

    fn on_event(&mut self, _event: &AgentEvent) {
        self.events += 1;
    }
}

#[test]
fn library_target_exposes_the_noop_hooks() {
    fn accepts<T: TurnHooks>() {}
    accepts::<niucode::NoHooks>();

    let mut prompt = String::from("base");
    niucode::NoHooks.augment_system_prompt(&mut prompt);
    assert_eq!(prompt, "base");
}

#[tokio::test]
async fn exec_invokes_the_hooks() {
    let directory = tempfile::tempdir().expect("tempdir");
    std::fs::write(directory.path().join("a.txt"), "hooked\n").expect("fixture file");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../niucode-agent/tests/fixtures/golden-read.json");

    let options = ExecOptions {
        prompt: "read a.txt".into(),
        cwd: Some(directory.path().to_path_buf()),
        session_dir: Some(directory.path().join("sessions")),
        replay: Some(fixture),
        sandbox: "full-access".into(),
        network: "on".into(),
        approval: "never".into(),
        json: true,
        max_steps: 2,
        ..ExecOptions::default()
    };
    let mut hooks = Recorder::default();
    let code = niucode::run_exec_with_hooks(options, &mut hooks)
        .await
        .expect("hooked exec");
    assert_eq!(code, 0);
    assert!(hooks.saw_prompt);
    assert!(hooks.events > 0);
    assert_eq!(hooks.turns, 1);
}

#[test]
fn tui_options_have_safe_defaults() {
    let options = TuiOptions::default();
    assert_eq!(options.preset, "deepseek");
    assert_eq!(options.sandbox, "workspace-write");
    assert_eq!(options.approval, "on-failure");
    assert_eq!(options.context_budget, 0);
    let _future = niucode::run_tui_with_hooks(TuiOptions::default(), Box::new(niucode::NoHooks));
    drop(_future);
}
