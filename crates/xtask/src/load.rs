use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use walkdir::WalkDir;

use crate::model::{
    CardFront, CardOut, Dashboard, DocFront, DocOut, Meta, ModuleOut, ProjectSpec, SourceFile,
};
use crate::render;
use crate::validate;

pub fn build(root: &Path) -> Result<Dashboard> {
    let spec = read_spec(root)?;
    let cards = load_cards(root)?;
    let docs = load_docs(root)?;

    validate::validate(root, &spec, &cards, &docs)?;

    let modules = spec
        .modules
        .iter()
        .map(|module| ModuleOut {
            module: module.clone(),
            sources: read_sources(root, &module.paths),
        })
        .collect::<Vec<_>>();

    let meta = Meta {
        schema_version: spec.schema_version,
        cards: cards.len(),
        modules: modules.len(),
        docs: docs.len(),
        generator: "cargo xtask dashboard".into(),
    };

    Ok(Dashboard {
        project: spec.project,
        milestones: spec.milestones,
        layers: spec.layers,
        modules,
        relations: spec.relations,
        cards,
        docs,
        events: spec.events,
        meta,
    })
}

fn read_spec(root: &Path) -> Result<ProjectSpec> {
    let path = root.join("docs/dashboard/project.json");
    let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

fn load_cards(root: &Path) -> Result<Vec<CardOut>> {
    let board_dir = root.join("docs/board");
    if !board_dir.is_dir() {
        bail!("missing {}", board_dir.display());
    }

    let mut paths = Vec::new();
    for entry in
        fs::read_dir(&board_dir).with_context(|| format!("read {}", board_dir.display()))?
    {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            paths.push(path);
        }
    }
    paths.sort();

    let mut cards = Vec::new();
    for path in paths {
        let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let (front, body) = split_frontmatter(&raw)
            .with_context(|| format!("parse frontmatter in {}", path.display()))?;
        let front = front.with_context(|| {
            format!(
                "{}: cards require TOML frontmatter between +++ lines",
                path.display()
            )
        })?;
        let front: CardFront = toml::from_str(&front)
            .with_context(|| format!("invalid card frontmatter in {}", path.display()))?;

        cards.push(CardOut {
            front,
            body_html: render::markdown(&body),
            path: relative(root, &path),
        });
    }

    cards.sort_by(|a, b| a.front.id.cmp(&b.front.id));
    Ok(cards)
}

fn load_docs(root: &Path) -> Result<Vec<DocOut>> {
    let docs_dir = root.join("docs");
    let board_dir = docs_dir.join("board");
    let dashboard_dir = docs_dir.join("dashboard");

    let walker = WalkDir::new(&docs_dir).into_iter().filter_entry(|entry| {
        let path = entry.path();
        !(path.starts_with(&board_dir) || path.starts_with(&dashboard_dir))
    });

    let mut docs = Vec::new();
    for entry in walker {
        let entry = entry.with_context(|| format!("walk {}", docs_dir.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }

        let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let (front, body) = split_frontmatter(&raw)
            .with_context(|| format!("parse frontmatter in {}", path.display()))?;
        let front = front.with_context(|| {
            format!(
                "{}: docs require TOML frontmatter between +++ lines",
                path.display()
            )
        })?;
        let front: DocFront = toml::from_str(&front)
            .with_context(|| format!("invalid doc frontmatter in {}", path.display()))?;

        let excerpt = front
            .summary
            .clone()
            .filter(|summary| !summary.trim().is_empty())
            .unwrap_or_else(|| render::excerpt(&body, 180));

        docs.push(DocOut {
            front,
            body_html: render::markdown(&body),
            path: relative(root, path),
            excerpt,
        });
    }

    docs.sort_by(|a, b| {
        a.front
            .order
            .cmp(&b.front.order)
            .then_with(|| a.front.id.cmp(&b.front.id))
    });
    Ok(docs)
}

fn read_sources(root: &Path, paths: &[String]) -> Vec<SourceFile> {
    let mut sources = Vec::new();

    for relative_path in paths {
        let path = root.join(relative_path);
        if !path.is_file() {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let (text, truncated) = clip(&raw, 200, 30_000);

        sources.push(SourceFile {
            path: relative_path.clone(),
            language: language(relative_path).into(),
            text,
            truncated,
        });
    }

    sources
}

fn split_frontmatter(raw: &str) -> Result<(Option<String>, String)> {
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let mut lines = raw.lines();

    if lines.next() != Some("+++") {
        return Ok((None, raw.to_string()));
    }

    let mut front = Vec::new();
    let mut body = Vec::new();
    let mut in_front = true;

    for line in lines {
        if in_front {
            if line == "+++" {
                in_front = false;
            } else {
                front.push(line);
            }
        } else {
            body.push(line);
        }
    }

    if in_front {
        bail!("unterminated frontmatter");
    }

    Ok((Some(front.join("\n")), body.join("\n")))
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn language(path: &str) -> &'static str {
    match PathBuf::from(path).extension().and_then(|ext| ext.to_str()) {
        Some("rs") => "rust",
        Some("toml") => "toml",
        Some("json") => "json",
        Some("md") => "markdown",
        Some("js") => "javascript",
        Some("css") => "css",
        Some("html") => "html",
        Some("sh") => "bash",
        Some("yml" | "yaml") => "yaml",
        _ => "text",
    }
}

fn clip(text: &str, max_lines: usize, max_bytes: usize) -> (String, bool) {
    let lines = text.lines().collect::<Vec<_>>();
    let take = lines.len().min(max_lines);
    let mut out = lines[..take].join("\n");
    let mut truncated = lines.len() > take;

    if out.len() > max_bytes {
        let mut cut = max_bytes;
        while cut > 0 && !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
        truncated = true;
    }

    if !out.is_empty() {
        out.push('\n');
    }
    (out, truncated)
}
