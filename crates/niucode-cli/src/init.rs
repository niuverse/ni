//! `/init` proposal generation and deterministic fallback.
//!
//! The model runs in its own read-only session. The proposal is stored in the
//! parent session as an event; applying it goes through Toolbox policy.

use std::fs;
use std::path::Path;

const REQUIRED: [&str; 6] = [
    "## Purpose",
    "## Commands",
    "## Architecture",
    "## Workflow",
    "## Guardrails",
    "## Current focus",
];

pub fn scan(cwd: &Path) -> String {
    let mut out = String::new();
    let name = cwd
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    out.push_str(&format!("project: {name}\n"));
    let mut entries = fs::read_dir(cwd)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
                .filter(|name| {
                    !matches!(
                        name.as_str(),
                        ".git" | "target" | "node_modules" | ".niucode" | ".agents"
                    ) && !name.starts_with(".env")
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    entries.sort();
    entries.truncate(40);
    out.push_str(&format!("top-level: {}\n", entries.join(", ")));

    let commands = commands(cwd);
    out.push_str("commands:\n");
    for command in &commands {
        out.push_str(&format!("- {command}\n"));
    }
    for (file, label) in [
        ("Cargo.toml", "Cargo.toml"),
        ("package.json", "package.json"),
        ("pyproject.toml", "pyproject.toml"),
        ("go.mod", "go.mod"),
        ("README.md", "README.md"),
        ("README", "README"),
    ] {
        if let Ok(text) = fs::read_to_string(cwd.join(file)) {
            let text = text.lines().take(60).collect::<Vec<_>>().join("\n");
            out.push_str(&format!("\n--- {label} (first 60 lines) ---\n{text}\n"));
        }
    }
    let existing = existing(cwd);
    if !existing.is_empty() {
        out.push_str("\n--- existing AGENTS.md ---\n");
        out.push_str(&existing);
    }
    out
}

pub fn existing(cwd: &Path) -> String {
    fs::read_to_string(cwd.join("AGENTS.md")).unwrap_or_default()
}

pub fn prompt(cwd: &Path, scan: &str) -> String {
    format!(
        "Generate a concise, project-specific `AGENTS.md` for the repository at {}.\n\n\
         Use read/grep/glob to inspect the repository. Do not modify any files. \
         The final assistant message must be the complete AGENTS.md file and nothing else: \
         no commentary, no code fences.\n\n\
         Required sections, in this order:\n\
         # AGENTS.md\n\
         ## Purpose\n\
         ## Commands\n\
         ## Architecture\n\
         ## Workflow\n\
         ## Guardrails\n\
         ## Current focus\n\n\
         Rules:\n\
         - Preserve useful manual content from an existing AGENTS.md; merge rather than discard.\n\
         - Commands must be real commands found in the project, not guesses.\n\
         - Keep it actionable for a coding agent: boundaries, verification, and non-goals.\n\
         - Soft target: 100 lines / 6 KiB. Do not pad.\n\
         - If you are unsure, inspect more; do not invent architecture.\n\n\
         Deterministic scan:\n{scan}\n",
        cwd.display()
    )
}

pub fn normalize(text: &str) -> Option<String> {
    let mut text = text.trim().to_string();
    if let Some(rest) = text.strip_prefix("```") {
        let rest = rest
            .strip_prefix("markdown")
            .or_else(|| rest.strip_prefix("md"))
            .unwrap_or(rest);
        let rest = rest.strip_prefix('\n').unwrap_or(rest);
        text = rest.trim_end_matches("```").trim().to_string();
    }
    if !text.starts_with("# AGENTS.md") {
        text = format!("# AGENTS.md\n\n{text}");
    }
    if REQUIRED.iter().all(|heading| text.contains(heading)) {
        Some(text)
    } else {
        None
    }
}

pub fn fallback(cwd: &Path, scan: &str) -> String {
    let previous = existing(cwd);
    let mut body = if previous.trim().is_empty() {
        format!(
            "# AGENTS.md\n\n\
             ## Purpose\n\n\
             Project guidance for coding agents working in `{}`.\n\n\
             ## Commands\n\n{}\n\
             ## Architecture\n\n\
             Use the existing module boundaries; read the relevant files before editing.\n\n\
             ## Workflow\n\n\
             1. Inspect the repository and relevant tests before changing code.\n\
             2. Keep changes small and scoped; preserve existing conventions.\n\
             3. Run the relevant checks before claiming success.\n\n\
             ## Guardrails\n\n\
             - Read a file before editing or overwriting it.\n\
             - Do not widen the tool or network surface without an explicit reason.\n\
             - Do not rewrite the event log or invent durable state outside it.\n\n\
             ## Current focus\n\n\
             No active focus recorded.\n",
            cwd.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("project"),
            commands(cwd)
                .iter()
                .map(|command| format!("- {command}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    } else {
        previous
    };
    for heading in REQUIRED {
        if !body.contains(heading) {
            body.push_str(&format!(
                "\n{heading}\n\n<!-- Add project-specific guidance. -->\n"
            ));
        }
    }
    if !body.starts_with("# AGENTS.md") {
        body = format!("# AGENTS.md\n\n{body}");
    }
    let _ = scan;
    body
}

pub fn size_warning(text: &str) -> Option<String> {
    let lines = text.lines().count();
    let bytes = text.len();
    (lines > 100 || bytes > 6 * 1024)
        .then(|| format!("proposal is larger than the soft target: {lines} lines / {bytes} bytes"))
}

fn commands(cwd: &Path) -> Vec<String> {
    if cwd.join("Cargo.toml").is_file() {
        return vec![
            "Test: `cargo test --workspace`".into(),
            "Build: `cargo build --workspace`".into(),
            "Lint: `cargo clippy --workspace --all-targets -- -D warnings`".into(),
        ];
    }
    if cwd.join("package.json").is_file() {
        return vec![
            "Test: `npm test`".into(),
            "Build: `npm run build`".into(),
            "Lint: `npm run lint`".into(),
        ];
    }
    if cwd.join("pyproject.toml").is_file() {
        return vec!["Test: `pytest`".into(), "Lint: `ruff check .`".into()];
    }
    if cwd.join("go.mod").is_file() {
        return vec![
            "Test: `go test ./...`".into(),
            "Build: `go build ./...`".into(),
        ];
    }
    vec![
        "Test: <!-- add the test command -->".into(),
        "Build: <!-- add the build command -->".into(),
        "Lint: <!-- add the lint command -->".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("niucode-init-{tag}-{stamp}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("temp dir");
        path
    }

    #[test]
    fn fallback_has_all_required_sections() {
        let directory = temp_dir("fallback");
        fs::write(directory.join("Cargo.toml"), "[package]\nname = \"x\"\n").expect("cargo");
        let text = fallback(&directory, "");
        for heading in REQUIRED {
            assert!(text.contains(heading), "missing {heading}");
        }
        assert!(text.contains("cargo test --workspace"));
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn normalize_strips_fences_and_requires_sections() {
        let raw = "```markdown\n# AGENTS.md\n## Purpose\n## Commands\n## Architecture\n## Workflow\n## Guardrails\n## Current focus\n```";
        let normalized = normalize(raw).expect("normalized");
        assert!(!normalized.contains("```"));
        assert!(normalize("too short").is_none());
    }
}
