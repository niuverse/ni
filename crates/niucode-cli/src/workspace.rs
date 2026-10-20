//! Global per-project workspace layout.
//!
//! niucode intentionally does not create a `.niucode` directory in the project
//! root. All state lives under `~/.niucode/projects/<key>/`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use niucode_core::now_ms;
use serde::{Deserialize, Serialize};

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct Workspace {
    pub project_key: String,
    pub project_dir: PathBuf,
    pub cwd: PathBuf,
    pub sessions: PathBuf,
    pub proposals: PathBuf,
    pub backups: PathBuf,
    pub extensions: PathBuf,
    pub cache: PathBuf,
    pub worktrees: PathBuf,
    pub config: PathBuf,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default)]
    pub defaults: ConfigDefaults,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigDefaults {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub context_budget: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProjectMeta {
    schema_version: u32,
    cwd: String,
    first_seen: u64,
    last_seen: u64,
}

impl Workspace {
    pub fn discover(cwd: &Path) -> Result<Self> {
        let cwd = cwd
            .canonicalize()
            .with_context(|| format!("resolve workspace cwd {}", cwd.display()))?;
        let root = global_root()?;
        Self::from_root(root, cwd)
    }

    fn from_root(root: PathBuf, cwd: PathBuf) -> Result<Self> {
        let project_key = project_key(&cwd);
        let project_dir = root.join("projects").join(&project_key);
        let config = root.join("config.toml");
        let workspace = Self {
            sessions: project_dir.join("sessions"),
            proposals: project_dir.join("proposals"),
            backups: project_dir.join("backups"),
            extensions: project_dir.join("extensions"),
            cache: project_dir.join("cache"),
            worktrees: project_dir.join("worktrees"),
            config,
            project_key,
            project_dir,
            cwd,
        };
        workspace.ensure()?;
        Ok(workspace)
    }

    fn ensure(&self) -> Result<()> {
        for directory in [
            &self.project_dir,
            &self.sessions,
            &self.proposals,
            &self.backups,
            &self.extensions,
            &self.cache,
            &self.worktrees,
        ] {
            fs::create_dir_all(directory)
                .with_context(|| format!("create workspace directory {}", directory.display()))?;
        }
        if !self.config.is_file() {
            fs::write(
                &self.config,
                "# niucode global defaults; CLI flags override.\n[defaults]\n# provider = \"deepseek\"\n# model = \"deepseek-flash\"\n# context_budget = 0\n",
            )
            .with_context(|| format!("write {}", self.config.display()))?;
        }
        let meta_path = self.project_dir.join("project.toml");
        let previous = fs::read_to_string(&meta_path)
            .ok()
            .and_then(|text| toml::from_str::<ProjectMeta>(&text).ok());
        let now = now_ms();
        let meta = ProjectMeta {
            schema_version: SCHEMA_VERSION,
            cwd: self.cwd.display().to_string(),
            first_seen: previous.as_ref().map(|meta| meta.first_seen).unwrap_or(now),
            last_seen: now,
        };
        let text = toml::to_string(&meta).context("serialize project.toml")?;
        fs::write(&meta_path, text)
            .with_context(|| format!("write project metadata {}", meta_path.display()))?;
        Ok(())
    }

    pub fn extension_workspace(&self) -> &Path {
        &self.cwd
    }
}

pub fn load_config() -> Result<GlobalConfig> {
    let path = global_root()?.join("config.toml");
    if !path.is_file() {
        return Ok(GlobalConfig::default());
    }
    let text =
        fs::read_to_string(&path).with_context(|| format!("read config {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse config {}", path.display()))
}

pub(crate) fn global_root() -> Result<PathBuf> {
    if let Ok(home) = std::env::var("NIUCODE_HOME") {
        if !home.trim().is_empty() {
            return Ok(PathBuf::from(home));
        }
    }
    let Some(home) = std::env::var_os("HOME") else {
        bail!("cannot determine niucode workspace: HOME is not set (set NIUCODE_HOME)");
    };
    Ok(PathBuf::from(home).join(".niucode"))
}

fn project_key(cwd: &Path) -> String {
    let basename = cwd
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("project");
    let mut safe = String::new();
    for character in basename.chars().take(40) {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
            safe.push(character);
        } else {
            safe.push('-');
        }
    }
    if safe.is_empty() {
        safe.push_str("project");
    }
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in cwd.as_os_str().to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{safe}-{hash:08x}")
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("niucode-workspace-{tag}-{stamp}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("temp dir");
        path
    }

    #[test]
    fn project_keys_are_stable_and_readable() {
        let path = Path::new("/tmp/My Project");
        let key = project_key(path);
        assert!(key.starts_with("My-Project-"));
        assert_eq!(key, project_key(path));
        assert_ne!(key, project_key(Path::new("/tmp/My-Project")));
    }

    #[test]
    fn workspace_never_creates_a_project_local_directory() {
        let root = temp_dir("root");
        let cwd = temp_dir("project");
        let workspace = Workspace::from_root(root.clone(), cwd.clone()).expect("workspace");
        assert!(!cwd.join(".niucode").exists());
        assert!(workspace.sessions.starts_with(root.join("projects")));
        assert!(workspace.project_dir.join("project.toml").is_file());
        assert!(!workspace.project_key.is_empty());
        assert!(workspace.project_key.contains("niucode-workspace-project"));
        fs::remove_dir_all(root).expect("cleanup root");
        fs::remove_dir_all(cwd).expect("cleanup cwd");
    }
}
