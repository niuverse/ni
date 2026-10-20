//! Codex-compatible skills discovery and trust.
//!
//! Skills are declarative prompt extensions, not code plugins. Global skills
//! under `~/.agents/skills` are trusted; project `.agents/skills` require an
//! explicit digest-bound trust entry.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::workspace::Workspace;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Global,
    Project,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Project => "project",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustEntry {
    pub name: String,
    pub digest: String,
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub scope: Scope,
    pub digest: String,
    pub trusted: bool,
}

#[derive(Debug, Clone)]
pub struct Registry {
    pub skills: Vec<Skill>,
    trust_path: PathBuf,
}

impl Registry {
    pub fn discover(workspace: &Workspace) -> Result<Self> {
        let global = global_skills_dir();
        let project = workspace.cwd.join(".agents/skills");
        let trust_path = workspace.project_dir.join("skills-trust.json");
        Self::discover_at(global.as_deref(), &project, &trust_path)
    }

    fn discover_at(global: Option<&Path>, project: &Path, trust_path: &Path) -> Result<Self> {
        let trusted = fs::read_to_string(trust_path)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<TrustEntry>>(&text).ok())
            .unwrap_or_default();
        let mut skills = Vec::new();
        if let Some(global) = global {
            skills.extend(discover_scope(global, Scope::Global, &trusted, true)?);
        }
        skills.extend(discover_scope(project, Scope::Project, &trusted, false)?);
        skills.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.scope.as_str().cmp(right.scope.as_str()))
        });
        Ok(Self {
            skills,
            trust_path: trust_path.to_path_buf(),
        })
    }

    pub fn list(&self) -> &[Skill] {
        &self.skills
    }

    pub fn find(&self, name: &str) -> Option<&Skill> {
        self.skills
            .iter()
            .find(|skill| skill.name == name)
            .or_else(|| self.skills.iter().rev().find(|skill| skill.name == name))
    }

    /// Returns the activation block and the read-only roots that must be
    /// available to the `read` tool so the body can be loaded on demand.
    pub fn activate(&self, text: &str) -> (String, Vec<PathBuf>) {
        let mut blocks = Vec::new();
        let mut roots = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for token in text.split(|character: char| character.is_whitespace()) {
            let Some(name) = token.strip_prefix('$') else {
                continue;
            };
            let name = name.trim_matches(|character: char| {
                !character.is_ascii_alphanumeric() && character != '-' && character != '_'
            });
            if name.is_empty() || !seen.insert(name.to_string()) {
                continue;
            }
            let Some(skill) = self.find(name) else {
                continue;
            };
            if !skill.trusted {
                continue;
            }
            blocks.push(format!(
                "## Skill: {}\nPath: {}\nDescription: {}\nRead this skill file with the read tool before applying it. Do not execute bundled scripts automatically.",
                skill.name,
                skill.path.display(),
                skill.description
            ));
            if let Some(parent) = skill.path.parent() {
                roots.push(parent.to_path_buf());
            }
        }
        if blocks.is_empty() {
            (String::new(), roots)
        } else {
            (
                format!("\n\n# Activated skills\n\n{}", blocks.join("\n\n")),
                roots,
            )
        }
    }

    pub fn trust(&self, name: &str) -> Result<()> {
        let skill = self
            .skills
            .iter()
            .find(|skill| skill.name == name)
            .with_context(|| format!("unknown skill {name:?}"))?;
        if skill.scope != Scope::Project {
            bail!("global skills are already trusted");
        }
        let mut entries = self.read_trust()?;
        entries.retain(|entry| entry.name != name);
        entries.push(TrustEntry {
            name: name.to_string(),
            digest: skill.digest.clone(),
        });
        self.write_trust(&entries)
    }

    pub fn untrust(&self, name: &str) -> Result<()> {
        let mut entries = self.read_trust()?;
        let before = entries.len();
        entries.retain(|entry| entry.name != name);
        if entries.len() == before {
            bail!("project skill {name:?} is not trusted");
        }
        self.write_trust(&entries)
    }

    fn read_trust(&self) -> Result<Vec<TrustEntry>> {
        if !self.trust_path.is_file() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(&self.trust_path)
            .with_context(|| format!("read {}", self.trust_path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("decode {}", self.trust_path.display()))
    }

    fn write_trust(&self, entries: &[TrustEntry]) -> Result<()> {
        if let Some(parent) = self.trust_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(entries)?;
        fs::write(&self.trust_path, format!("{text}\n"))
            .with_context(|| format!("write {}", self.trust_path.display()))
    }
}

fn global_skills_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".agents/skills"))
}

fn discover_scope(
    root: &Path,
    scope: Scope,
    trusted: &[TrustEntry],
    default_trusted: bool,
) -> Result<Vec<Skill>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut skills = Vec::new();
    for entry in fs::read_dir(root).with_context(|| format!("read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        let skill_file = if path.is_dir() {
            path.join("SKILL.md")
        } else if path.extension().is_some_and(|extension| extension == "md") {
            path.clone()
        } else {
            continue;
        };
        if !skill_file.is_file() {
            continue;
        }
        let Some((name, description, body)) = parse_skill(&skill_file)? else {
            continue;
        };
        let name = name.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("skill")
                .to_string()
        });
        let digest = digest(&format!("{name}\n{description}\n{body}"));
        let trusted = default_trusted
            || trusted
                .iter()
                .any(|entry| entry.name == name && entry.digest == digest);
        skills.push(Skill {
            name,
            description,
            path: skill_file,
            scope,
            digest,
            trusted,
        });
    }
    Ok(skills)
}

fn parse_skill(path: &Path) -> Result<Option<(Option<String>, String, String)>> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let Some(rest) = text.strip_prefix("---") else {
        return Ok(None);
    };
    let Some(end) = rest.find("\n---") else {
        return Ok(None);
    };
    let frontmatter = &rest[..end];
    let body_start = end + 4;
    let body = text.get(body_start..).unwrap_or("").trim().to_string();
    let mut name = None;
    let mut description = String::new();
    for line in frontmatter.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        match key.trim() {
            "name" => name = Some(value),
            "description" if description.is_empty() => description = value,
            _ => {}
        }
    }
    Ok(Some((name, description, body)))
}

fn digest(text: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("niucode-skills-{tag}-{stamp}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("temp");
        path
    }

    fn write_skill(root: &Path, name: &str, description: &str) {
        let directory = root.join(name);
        fs::create_dir_all(&directory).expect("skill dir");
        fs::write(
            directory.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n"),
        )
        .expect("skill");
    }

    #[test]
    fn project_skills_require_digest_trust() {
        let global = temp_dir("global");
        let project = temp_dir("project");
        let trust = temp_dir("trust").join("trust.json");
        write_skill(&global, "rust", "Rust guidance");
        write_skill(&project, "local", "Project guidance");
        let registry = Registry::discover_at(Some(&global), &project, &trust).expect("registry");
        assert!(registry.find("rust").expect("global").trusted);
        assert!(!registry.find("local").expect("local").trusted);
        let (block, roots) = registry.activate("use $rust and $local");
        assert!(block.contains("$rust") || block.contains("## Skill: rust"));
        assert!(!block.contains("## Skill: local"));
        assert_eq!(roots.len(), 1);

        registry.trust("local").expect("trust");
        let registry = Registry::discover_at(Some(&global), &project, &trust).expect("reload");
        assert!(registry.find("local").expect("local").trusted);
        let (block, _) = registry.activate("$local");
        assert!(block.contains("## Skill: local"));
        assert!(registry.untrust("local").is_ok());
        let _ = fs::remove_dir_all(global);
        let _ = fs::remove_dir_all(project);
        let _ = fs::remove_dir_all(Path::new(&trust).parent().unwrap());
    }
}
