//! Layered `AGENTS.md` discovery.
//!
//! Instructions are discovered from the working directory up to the filesystem
//! root, nearest first. They are appended after the stable system prompt so the
//! prefix stays cache-friendly and changes only when a file changes.

use std::fs;
use std::path::Path;

const MAX_FILE_BYTES: usize = 32 * 1024;
const MAX_TOTAL_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Instructions {
    /// Merged instruction text, empty when no `AGENTS.md` was found.
    pub text: String,
    /// Absolute paths that contributed text, nearest first.
    pub sources: Vec<String>,
    /// Stable FNV-1a digest of the merged text, used to explain cache changes.
    pub digest: String,
    /// Read or truncation problems worth surfacing.
    pub warnings: Vec<String>,
}

pub fn load(cwd: &Path) -> Instructions {
    let mut instructions = Instructions::default();
    let mut sections = Vec::new();
    let mut total_bytes = 0_usize;
    let mut omitted = false;

    for directory in cwd.ancestors() {
        let path = directory.join("AGENTS.md");
        if !path.is_file() {
            continue;
        }

        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                instructions
                    .warnings
                    .push(format!("cannot read {}: {error}", path.display()));
                continue;
            }
        };
        if bytes.len() > MAX_FILE_BYTES {
            instructions.warnings.push(format!(
                "{} is larger than {MAX_FILE_BYTES} bytes; truncated",
                path.display()
            ));
        }
        let bytes = &bytes[..bytes.len().min(MAX_FILE_BYTES)];
        let content = String::from_utf8_lossy(bytes).trim().to_string();
        if content.is_empty() {
            continue;
        }

        let section = format!("### {}\n\n{content}\n\n", path.display());
        if total_bytes + section.len() > MAX_TOTAL_BYTES {
            omitted = true;
            break;
        }
        total_bytes += section.len();
        instructions.sources.push(path.display().to_string());
        sections.push(section);
    }

    if sections.is_empty() {
        return instructions;
    }

    let mut text = String::new();
    text.push_str("## Repository instructions\n\n");
    text.push_str(
        "The following AGENTS.md files apply to this workspace. Nearer files are listed first and take precedence when instructions conflict.\n\n",
    );
    for section in sections {
        text.push_str(&section);
    }
    if omitted {
        text.push_str(
            "_Additional AGENTS.md files were omitted because the instruction budget was exhausted._\n",
        );
    }

    instructions.digest = digest(&text);
    instructions.text = text;
    instructions
}

/// Compose the stable base prompt with discovered instructions.
pub fn compose_system_prompt(base: &str, instructions: &str) -> String {
    if instructions.is_empty() {
        base.to_string()
    } else {
        format!("{base}\n\n{instructions}")
    }
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
            std::env::temp_dir().join(format!("niucode-instructions-{tag}-{stamp}-{sequence}"));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create temp dir");
        directory
    }

    #[test]
    fn nearest_file_is_listed_first() {
        let root = temp_dir("layers");
        let child = root.join("crates/app");
        fs::create_dir_all(&child).expect("child");
        fs::write(root.join("AGENTS.md"), "root rule\n").expect("root rule");
        fs::write(child.join("AGENTS.md"), "child rule\n").expect("child rule");

        let loaded = load(&child);
        assert!(loaded.sources.len() >= 2);
        assert!(loaded.sources[0].ends_with("crates/app/AGENTS.md"));
        assert!(loaded.sources[1].ends_with("AGENTS.md"));
        let child_position = loaded.text.find("child rule").expect("child");
        let root_position = loaded.text.find("root rule").expect("root");
        assert!(child_position < root_position);
        assert_eq!(loaded.digest.len(), 16);

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn modifying_a_file_changes_the_digest() {
        let directory = temp_dir("change");
        let path = directory.join("AGENTS.md");
        fs::write(&path, "first\n").expect("write");
        let first = load(&directory);
        fs::write(&path, "second\n").expect("rewrite");
        let second = load(&directory);
        assert_eq!(first.sources, second.sources);
        assert_ne!(first.digest, second.digest);
        assert!(second.text.contains("second"));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn empty_files_are_not_sources() {
        let directory = temp_dir("empty");
        let path = directory.join("AGENTS.md");
        assert!(!load(&directory)
            .sources
            .contains(&path.display().to_string()));
        fs::write(&path, "  \n").expect("write");
        let loaded = load(&directory);
        assert!(!loaded.sources.contains(&path.display().to_string()));

        fs::remove_dir_all(directory).expect("cleanup");
    }
}
