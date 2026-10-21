use std::path::{Path,PathBuf};
pub fn excluded(path:&Path,rules:&[PathBuf])->bool {
    rules.iter().any(|rule|path.to_string_lossy().starts_with(rule.to_string_lossy().as_ref()))
}
