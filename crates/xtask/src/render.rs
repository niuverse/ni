use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use pulldown_cmark::{html, Options, Parser};
use serde::Serialize;

use crate::model::Dashboard;

pub fn markdown(input: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(input, options);
    let mut output = String::new();
    html::push_html(&mut output, parser);
    output
}

pub fn excerpt(markdown: &str, limit: usize) -> String {
    let mut text = String::new();

    for line in markdown.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with("```")
            || line.starts_with('|')
            || line.starts_with("---")
        {
            continue;
        }

        let cleaned =
            line.trim_start_matches(|c: char| matches!(c, '-' | '*' | '>' | ' ' | '1'..='9' | '.'));
        text.push_str(cleaned.trim());
        text.push(' ');

        if text.chars().count() >= limit {
            break;
        }
    }

    let mut out = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.chars().count() > limit {
        out = out.chars().take(limit).collect();
        out.push('…');
    }
    out
}

pub fn write_dashboard(root: &Path, dashboard: &Dashboard, check: bool) -> Result<()> {
    let template_path = root.join("docs/dashboard/template.html");
    let style_path = root.join("docs/dashboard/style.css");
    let app_path = root.join("docs/dashboard/app.js");

    let template = fs::read_to_string(&template_path)
        .with_context(|| format!("read {}", template_path.display()))?;
    let style = fs::read_to_string(&style_path)
        .with_context(|| format!("read {}", style_path.display()))?;
    let app =
        fs::read_to_string(&app_path).with_context(|| format!("read {}", app_path.display()))?;

    let data = serde_json::to_string(dashboard).context("serialize dashboard data")?;
    let data = escape_for_script(&data);

    // Replace everything except DATA before DATA: the embedded source of
    // template.html contains the placeholders themselves, and replacing into
    // already-inserted JSON would corrupt it.
    let html = template
        .replace("{{STYLE}}", &style)
        .replace("{{APP}}", &app)
        .replace("{{DATA}}", &data);

    let index_path = root.join("docs/index.html");
    let model_path = root.join("docs/dashboard/model.json");
    let model = serde_json::to_string_pretty(dashboard).context("serialize dashboard model")?;
    let model = format!("{model}\n");

    if check {
        let current_html = fs::read_to_string(&index_path).unwrap_or_default();
        let current_model = fs::read_to_string(&model_path).unwrap_or_default();
        if current_html != html || current_model != model {
            bail!("dashboard is stale; run `cargo xtask dashboard`");
        }
        println!("dashboard is up to date");
        return Ok(());
    }

    fs::write(&index_path, &html).with_context(|| format!("write {}", index_path.display()))?;
    fs::write(&model_path, &model).with_context(|| format!("write {}", model_path.display()))?;

    let receipt = Receipt {
        schema_version: dashboard.meta.schema_version,
        generated_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        git_revision: git_output(root, &["rev-parse", "--short", "HEAD"]),
        git_dirty: !git_output(root, &["status", "--porcelain"]).is_empty(),
        cards: dashboard.cards.len(),
        modules: dashboard.modules.len(),
        docs: dashboard.docs.len(),
        events: dashboard.events.len(),
        index_bytes: html.len(),
        model_bytes: model.len(),
        inputs: vec![
            "docs/dashboard/project.json".into(),
            "docs/board/*.md".into(),
            "docs/**/*.md".into(),
            "docs/dashboard/template.html".into(),
            "docs/dashboard/style.css".into(),
            "docs/dashboard/app.js".into(),
        ],
    };
    let receipt_path = root.join("docs/dashboard/receipt.json");
    let receipt_json = serde_json::to_string_pretty(&receipt)?;
    fs::write(&receipt_path, format!("{receipt_json}\n"))
        .with_context(|| format!("write {}", receipt_path.display()))?;

    println!(
        "dashboard: {} cards, {} modules, {} docs → docs/index.html ({:.1} KiB)",
        dashboard.cards.len(),
        dashboard.modules.len(),
        dashboard.docs.len(),
        html.len() as f64 / 1024.0
    );
    Ok(())
}

#[derive(Debug, Serialize)]
struct Receipt {
    schema_version: u32,
    generated_at_unix: u64,
    git_revision: String,
    git_dirty: bool,
    cards: usize,
    modules: usize,
    docs: usize,
    events: usize,
    index_bytes: usize,
    model_bytes: usize,
    inputs: Vec<String>,
}

fn git_output(root: &Path, args: &[&str]) -> String {
    Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default()
}

fn escape_for_script(json: &str) -> String {
    json.replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}
