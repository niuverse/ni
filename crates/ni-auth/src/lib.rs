//! Credential resolution for ni.
//!
//! Resolution order is deliberately small: environment variable, then OS
//! keyring, then a 0600 credentials file. Commands never print the secret.

mod error;
mod file;
mod keyring_store;

pub use error::AuthError;

use std::env;
use std::fmt;
use std::path::{Path, PathBuf};

use file::FileStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Env,
    Keyring,
    File,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Env => "env",
            Self::Keyring => "keyring",
            Self::File => "file",
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Storage {
    Keyring,
    File,
}

impl fmt::Display for Storage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Keyring => formatter.write_str("OS keyring"),
            Self::File => formatter.write_str("0600 credentials file"),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SecretKey(String);

impl SecretKey {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretKey(***)")
    }
}

#[derive(Debug)]
pub struct Resolved {
    pub key: SecretKey,
    pub source: Source,
}

#[derive(Debug, Clone)]
pub struct AuthStatus {
    pub provider: String,
    pub env: bool,
    pub keyring: bool,
    pub file: bool,
    pub file_private: bool,
    pub effective: Option<Source>,
}

pub struct AuthStore {
    file: FileStore,
    keyring_enabled: bool,
}

impl AuthStore {
    pub fn new() -> Result<Self, AuthError> {
        let directory = config_dir()?;
        let keyring_enabled = !matches!(env::var("NI_AUTH_DISABLE_KEYRING").as_deref(), Ok("1"));

        Ok(Self {
            file: FileStore::new(directory.join("credentials.toml")),
            keyring_enabled,
        })
    }

    /// Test/local helper: use a specific config directory and skip the keyring.
    pub fn at(directory: &Path) -> Self {
        Self {
            file: FileStore::new(directory.join("credentials.toml")),
            keyring_enabled: false,
        }
    }

    pub fn credentials_path(&self) -> &Path {
        self.file.path()
    }

    pub fn resolve(&self, provider: &str) -> Result<Option<Resolved>, AuthError> {
        let env_value = env_var(provider).and_then(|name| env::var(name).ok());
        self.resolve_with(provider, env_value)
    }

    pub fn resolve_with(
        &self,
        provider: &str,
        env_value: Option<String>,
    ) -> Result<Option<Resolved>, AuthError> {
        if let Some(value) = env_value
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            return Ok(Some(Resolved {
                key: SecretKey(value),
                source: Source::Env,
            }));
        }

        if self.keyring_enabled {
            if let Some(value) = keyring_store::get(provider) {
                return Ok(Some(Resolved {
                    key: SecretKey(value),
                    source: Source::Keyring,
                }));
            }
        }

        if let Some(value) = self.file.get(provider)? {
            return Ok(Some(Resolved {
                key: SecretKey(value),
                source: Source::File,
            }));
        }

        Ok(None)
    }

    pub fn status(&self, provider: &str) -> Result<AuthStatus, AuthError> {
        let env_value = env_var(provider).and_then(|name| env::var(name).ok());
        self.status_with(provider, env_value)
    }

    pub fn status_with(
        &self,
        provider: &str,
        env_value: Option<String>,
    ) -> Result<AuthStatus, AuthError> {
        let env = env_value
            .as_deref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        let keyring = if self.keyring_enabled {
            keyring_store::get(provider).is_some()
        } else {
            false
        };
        let file = self.file.get(provider)?.is_some();
        let file_private = if file { self.file.is_private() } else { true };

        let effective = if env {
            Some(Source::Env)
        } else if keyring {
            Some(Source::Keyring)
        } else if file {
            Some(Source::File)
        } else {
            None
        };

        Ok(AuthStatus {
            provider: provider.to_string(),
            env,
            keyring,
            file,
            file_private,
            effective,
        })
    }

    /// Store the key in the OS keyring, falling back to a 0600 file.
    pub fn set(&self, provider: &str, api_key: &str) -> Result<Storage, AuthError> {
        let api_key = api_key.trim();
        if api_key.is_empty() {
            return Err(AuthError::EmptyKey);
        }

        if self.keyring_enabled && keyring_store::set(provider, api_key) {
            // Avoid keeping a stale copy in the file when the keyring succeeded.
            let _ = self.file.remove(provider);
            return Ok(Storage::Keyring);
        }

        self.file.set(provider, api_key)?;
        Ok(Storage::File)
    }

    pub fn unset(&self, provider: &str) -> Result<bool, AuthError> {
        let keyring_removed = self.keyring_enabled && keyring_store::delete(provider);
        let file_removed = self.file.remove(provider)?;
        Ok(keyring_removed || file_removed)
    }
}

pub fn env_var(provider: &str) -> Option<&'static str> {
    if provider.eq_ignore_ascii_case("deepseek") {
        Some("DEEPSEEK_API_KEY")
    } else if provider.eq_ignore_ascii_case("openai") {
        Some("OPENAI_API_KEY")
    } else {
        None
    }
}

pub fn config_dir() -> Result<PathBuf, AuthError> {
    if let Some(directory) = env::var_os("NI_CONFIG_DIR")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
    {
        return Ok(directory);
    }

    if let Some(directory) = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
    {
        return Ok(directory.join("ni"));
    }

    if let Some(home) = env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
    {
        return Ok(home.join(".config").join("ni"));
    }

    if let Some(home) = env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
    {
        return Ok(home.join(".config").join("ni"));
    }

    Err(AuthError::NoConfigDir)
}

#[cfg(test)]
mod tests {
    use std::fs;
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
        let directory = env::temp_dir().join(format!("ni-auth-{tag}-{stamp}-{sequence}"));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create temp dir");
        directory
    }

    #[test]
    fn file_round_trip_and_status() {
        let directory = temp_dir("roundtrip");
        let store = AuthStore::at(&directory);

        assert!(store.set("deepseek", "sk-test").is_ok());
        let resolved = store
            .resolve_with("deepseek", None)
            .expect("resolve")
            .expect("credential");
        assert_eq!(resolved.key.expose(), "sk-test");
        assert_eq!(resolved.source, Source::File);

        let status = store.status_with("deepseek", None).expect("status");
        assert!(status.file);
        assert!(status.file_private);
        assert_eq!(status.effective, Some(Source::File));

        assert!(store.unset("deepseek").expect("unset"));
        assert!(store
            .resolve_with("deepseek", None)
            .expect("resolve")
            .is_none());

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn environment_wins_over_file() {
        let directory = temp_dir("env");
        let store = AuthStore::at(&directory);
        store.set("deepseek", "file-key").expect("set file");

        let resolved = store
            .resolve_with("deepseek", Some("env-key".into()))
            .expect("resolve")
            .expect("credential");
        assert_eq!(resolved.key.expose(), "env-key");
        assert_eq!(resolved.source, Source::Env);

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn corrupt_file_is_reported_without_content() {
        let directory = temp_dir("corrupt");
        let store = AuthStore::at(&directory);
        fs::write(store.credentials_path(), "not toml").expect("write fixture");

        let error = store
            .resolve_with("deepseek", None)
            .expect_err("corrupt file must fail");
        assert!(matches!(error, AuthError::CorruptCredentials));

        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn file_is_owner_read_write_only() {
        use std::os::unix::fs::PermissionsExt;

        let directory = temp_dir("permissions");
        let store = AuthStore::at(&directory);
        store.set("deepseek", "sk-test").expect("set file");

        let mode = fs::metadata(store.credentials_path())
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);

        fs::remove_dir_all(directory).expect("cleanup");
    }
}
