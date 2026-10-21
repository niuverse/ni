//! Explicit import of a literal shell assignment, without executing shell code.
use crate::AuthError;
use std::path::Path;

pub(crate) fn literal_assignment(path: &Path, name: &str) -> Result<Option<String>, AuthError> {
    let text = std::fs::read_to_string(path)?;
    let mut value = None;
    for line in text.lines() {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        let Some((key, raw)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != name {
            continue;
        }
        let raw = raw.trim();
        let parsed = if let Some(first) = raw.chars().next().filter(|c| *c == '\'' || *c == '"') {
            let tail = &raw[1..];
            let end = tail
                .find(first)
                .ok_or(AuthError::UnsupportedShellAssignment)?;
            let after = tail[end + 1..].trim();
            if !after.is_empty() && !after.starts_with('#') {
                return Err(AuthError::UnsupportedShellAssignment);
            }
            &tail[..end]
        } else {
            let token = raw.split_whitespace().next().unwrap_or("");
            let after = raw[token.len()..].trim();
            if !after.is_empty() && !after.starts_with('#') {
                return Err(AuthError::UnsupportedShellAssignment);
            }
            token
        };
        // API keys need none of shell's expansion/escape operators.
        if parsed.is_empty()
            || !parsed
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_./:+=".contains(&b))
        {
            return Err(AuthError::UnsupportedShellAssignment);
        }
        value = Some(parsed.to_owned());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imports_only_literals_and_never_expands_shell() {
        let path = std::env::temp_dir().join(format!("niucode-auth-shell-{}", std::process::id()));
        std::fs::write(
            &path,
            "echo unrelated\nexport DEEPSEEK_API_KEY='test-key' # comment\n",
        )
        .unwrap();
        assert_eq!(
            literal_assignment(&path, "DEEPSEEK_API_KEY").unwrap(),
            Some("test-key".into())
        );
        std::fs::write(
            &path,
            "export DEEPSEEK_API_KEY=\"$(touch /tmp/not-executed)\"\n",
        )
        .unwrap();
        assert!(literal_assignment(&path, "DEEPSEEK_API_KEY").is_err());
        std::fs::write(&path, "export DEEPSEEK_API_KEY=key unexpected\n").unwrap();
        assert!(literal_assignment(&path, "DEEPSEEK_API_KEY").is_err());
        std::fs::write(&path, "export DEEPSEEK_API_KEY='key'; echo surprise\n").unwrap();
        assert!(literal_assignment(&path, "DEEPSEEK_API_KEY").is_err());
        std::fs::remove_file(path).unwrap();
    }
}
