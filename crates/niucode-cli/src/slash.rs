//! Slash-command parsing for the interactive TUI.

use std::path::Path;

/// Slash commands surfaced by completion and `/help`.
pub const COMMANDS: &[(&str, &str)] = &[
    ("/init", "create AGENTS.md in the workspace"),
    ("/clear", "start a new session (old log is kept)"),
    ("/plan", "enter or leave read-only plan mode"),
    ("/goal", "set, run, stop, or clear the session goal"),
    ("/fork", "fork the current session [event-id]"),
    ("/status", "show model, sandbox, context, session and cache"),
    ("/session", "show the current session path and event count"),
    ("/last", "expand the most recent tool output"),
    ("/help", "show this help"),
    ("/exit", "leave the session"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub name: String,
    pub description: String,
}

/// Built-in command metadata. Plugin commands can be appended later; the TUI
/// menu, `/help`, and parsing should share this registry rather than keeping a
/// second command table.
#[derive(Debug, Clone, Default)]
pub struct CommandRegistry {
    entries: Vec<CommandSpec>,
}

impl CommandRegistry {
    pub fn builtin() -> Self {
        Self {
            entries: COMMANDS
                .iter()
                .map(|(name, description)| CommandSpec {
                    name: (*name).to_string(),
                    description: (*description).to_string(),
                })
                .collect(),
        }
    }

    pub fn all(&self) -> &[CommandSpec] {
        &self.entries
    }

    pub fn candidates(&self, prefix: &str) -> Vec<&CommandSpec> {
        self.entries
            .iter()
            .filter(|spec| spec.name.starts_with(prefix))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Last,
    Init,
    Clear,
    Plan(PlanAction),
    Goal(GoalAction),
    Fork(Option<String>),
    Status,
    Session,
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanAction {
    Enter,
    Exit,
    Approve,
    Status,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalAction {
    Show,
    Set(String),
    Run,
    Stop,
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parsed {
    Command(Command),
    Unknown(String),
    Usage(String),
    None,
}

pub fn parse(input: &str) -> Parsed {
    let trimmed = input.trim();
    if matches!(trimmed, "exit" | "quit") {
        return Parsed::Command(Command::Exit);
    }
    let (prefix, rest) = if let Some(rest) = trimmed.strip_prefix('/') {
        ('/', rest)
    } else if let Some(rest) = trimmed.strip_prefix(':') {
        (':', rest)
    } else {
        return Parsed::None;
    };

    let (name, rest) = rest
        .split_once(char::is_whitespace)
        .map(|(name, rest)| (name, rest.trim()))
        .unwrap_or((rest, ""));
    let args = rest.split_whitespace().collect::<Vec<_>>();

    if name == "goal" {
        let command = match rest {
            "" => Command::Goal(GoalAction::Show),
            "run" => Command::Goal(GoalAction::Run),
            "stop" | "pause" | "off" => Command::Goal(GoalAction::Stop),
            "clear" => Command::Goal(GoalAction::Clear),
            text => Command::Goal(GoalAction::Set(text.to_string())),
        };
        return Parsed::Command(command);
    }

    let command = match name {
        "help" | "?" => Command::Help,
        "last" => Command::Last,
        "init" => Command::Init,
        "clear" => Command::Clear,
        "plan" => match rest {
            "" => Command::Plan(PlanAction::Enter),
            "off" | "exit" => Command::Plan(PlanAction::Exit),
            "approve" => Command::Plan(PlanAction::Approve),
            "status" => Command::Plan(PlanAction::Status),
            _ => return Parsed::Usage(usage_plan()),
        },
        "fork" => Command::Fork(args.first().map(|arg| (*arg).to_string())),
        "status" => Command::Status,
        "session" => Command::Session,
        "exit" | "quit" | "q" => Command::Exit,
        _ if prefix == '/' => return Parsed::Unknown(name.to_string()),
        _ => return Parsed::None,
    };

    if args.len() > 1 {
        return Parsed::Usage(usage(&command));
    }
    if !args.is_empty()
        && !matches!(
            command,
            Command::Fork(_) | Command::Plan(_) | Command::Goal(_)
        )
    {
        return Parsed::Usage(usage(&command));
    }

    Parsed::Command(command)
}

fn usage(command: &Command) -> String {
    match command {
        Command::Help => "/help".into(),
        Command::Last => "/last".into(),
        Command::Init => "/init".into(),
        Command::Clear => "/clear".into(),
        Command::Plan(action) => match action {
            PlanAction::Enter => "/plan".into(),
            PlanAction::Exit => "/plan off".into(),
            PlanAction::Approve => "/plan approve".into(),
            PlanAction::Status => "/plan status".into(),
        },
        Command::Goal(_) => "/goal [condition|run|stop|clear]".into(),
        Command::Fork(_) => "/fork [event-id]".into(),
        Command::Status => "/status".into(),
        Command::Session => "/session".into(),
        Command::Exit => "/exit".into(),
    }
}

fn usage_plan() -> String {
    "/plan [off|approve|status]".into()
}

/// Build a small, repo-aware AGENTS.md scaffold.
pub fn init_template(cwd: &Path) -> String {
    let name = cwd
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    let commands = if cwd.join("Cargo.toml").is_file() {
        "- Test: `cargo test --workspace`\n\
         - Build: `cargo build --workspace`\n\
         - Lint: `cargo clippy --workspace --all-targets -- -D warnings`"
            .to_string()
    } else if cwd.join("package.json").is_file() {
        "- Test: `npm test`\n- Build: `npm run build`\n- Lint: `npm run lint`".to_string()
    } else if cwd.join("pyproject.toml").is_file() {
        "- Test: `pytest`\n- Lint: `ruff check .`".to_string()
    } else if cwd.join("go.mod").is_file() {
        "- Test: `go test ./...`\n- Build: `go build ./...`".to_string()
    } else {
        "- Test: <!-- add the test command -->\n\
         - Build: <!-- add the build command -->\n\
         - Lint: <!-- add the lint command -->"
            .to_string()
    };

    format!(
        "# AGENTS.md\n\n\
         <!-- Generated by niucode /init; edit freely. -->\n\n\
         ## Project\n\n\
         `{name}`\n\n\
         ## Commands\n\n\
         {commands}\n\n\
         ## Conventions\n\n\
         - Read a file before editing or overwriting it.\n\
         - Keep changes small and run the relevant checks before claiming success.\n"
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("niucode-slash-{tag}-{stamp}-{sequence}"));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create temp dir");
        directory
    }

    #[test]
    fn parses_known_slash_and_colon_commands() {
        assert_eq!(parse("/help"), Parsed::Command(Command::Help));
        assert_eq!(parse(":help"), Parsed::Command(Command::Help));
        assert_eq!(parse("/q"), Parsed::Command(Command::Exit));
        assert_eq!(parse(":q"), Parsed::Command(Command::Exit));
        assert_eq!(parse("exit"), Parsed::Command(Command::Exit));
        assert_eq!(parse("quit"), Parsed::Command(Command::Exit));
        assert_eq!(parse("/fork"), Parsed::Command(Command::Fork(None)));
        assert_eq!(
            parse("/fork e000012"),
            Parsed::Command(Command::Fork(Some("e000012".into())))
        );
    }

    #[test]
    fn reports_unknown_and_usage_errors() {
        assert_eq!(parse("/nope"), Parsed::Unknown("nope".into()));
        assert_eq!(parse("/clear extra"), Parsed::Usage("/clear".into()));
        assert_eq!(
            parse("/fork e1 e2"),
            Parsed::Usage("/fork [event-id]".into())
        );
    }

    #[test]
    fn leaves_plain_text_and_colon_prose_alone() {
        assert_eq!(parse("fix the test"), Parsed::None);
        assert_eq!(parse(":set number"), Parsed::None);
        assert_eq!(parse(""), Parsed::None);
    }

    #[test]
    fn registry_filters_commands() {
        let registry = CommandRegistry::builtin();
        let matches = registry.candidates("/cl");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "/clear");
        assert_eq!(registry.candidates("/pl")[0].name, "/plan");
        assert_eq!(registry.candidates("/g")[0].name, "/goal");
        assert!(registry.candidates("/nope").is_empty());
        assert_eq!(registry.all().len(), COMMANDS.len());
    }

    #[test]
    fn parses_plan_and_goal() {
        assert_eq!(
            parse("/plan"),
            Parsed::Command(Command::Plan(PlanAction::Enter))
        );
        assert_eq!(
            parse("/plan off"),
            Parsed::Command(Command::Plan(PlanAction::Exit))
        );
        assert_eq!(
            parse("/plan approve"),
            Parsed::Command(Command::Plan(PlanAction::Approve))
        );
        assert_eq!(
            parse("/goal all tests pass"),
            Parsed::Command(Command::Goal(GoalAction::Set("all tests pass".into())))
        );
        assert_eq!(
            parse("/goal"),
            Parsed::Command(Command::Goal(GoalAction::Show))
        );
        assert_eq!(
            parse("/goal clear"),
            Parsed::Command(Command::Goal(GoalAction::Clear))
        );
        assert_eq!(
            parse("/goal run"),
            Parsed::Command(Command::Goal(GoalAction::Run))
        );
        assert_eq!(
            parse("/goal stop"),
            Parsed::Command(Command::Goal(GoalAction::Stop))
        );
        assert_eq!(
            parse("/plan nope"),
            Parsed::Usage("/plan [off|approve|status]".into())
        );
    }

    #[test]
    fn init_template_detects_rust_projects() {
        let directory = temp_dir("rust");
        fs::write(directory.join("Cargo.toml"), "[package]\nname = \"x\"\n").expect("cargo");
        let template = init_template(&directory);
        assert!(template.contains("cargo test --workspace"));
        assert!(template.contains("## Conventions"));
        assert!(template.contains("Read a file before editing"));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn init_template_falls_back_for_unknown_projects() {
        let directory = temp_dir("generic");
        let template = init_template(&directory);
        assert!(template.contains("<!-- add the test command -->"));
        assert!(template.contains("## Commands"));

        fs::remove_dir_all(directory).expect("cleanup");
    }
}
