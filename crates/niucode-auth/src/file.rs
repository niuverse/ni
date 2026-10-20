use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::AuthError;

#[derive(Debug, Default, Serialize, Deserialize)]
struct Credentials {
    #[serde(default)]
    providers: BTreeMap<String, ProviderEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProviderEntry {
    api_key: String,
}

pub(crate) struct FileStore {
    path: PathBuf,
}

impl FileStore {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn get(&self, provider: &str) -> Result<Option<String>, AuthError> {
        let credentials = self.read()?;
        Ok(credentials
            .providers
            .get(provider)
            .map(|entry| entry.api_key.trim().to_string())
            .filter(|key| !key.is_empty()))
    }

    pub(crate) fn set(&self, provider: &str, api_key: &str) -> Result<(), AuthError> {
        let mut credentials = self.read()?;
        credentials.providers.insert(
            provider.to_string(),
            ProviderEntry {
                api_key: api_key.to_string(),
            },
        );
        self.write(&credentials)
    }

    pub(crate) fn remove(&self, provider: &str) -> Result<bool, AuthError> {
        let mut credentials = self.read()?;
        let removed = credentials.providers.remove(provider).is_some();
        if removed {
            self.write(&credentials)?;
        }
        Ok(removed)
    }

    pub(crate) fn is_private(&self) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            self.path
                .metadata()
                .map(|metadata| metadata.permissions().mode() & 0o077 == 0)
                .unwrap_or(true)
        }
        #[cfg(not(unix))]
        {
            true
        }
    }

    fn read(&self) -> Result<Credentials, AuthError> {
        if !self.path.exists() {
            return Ok(Credentials::default());
        }

        let raw = fs::read_to_string(&self.path)?;
        if raw.trim().is_empty() {
            return Ok(Credentials::default());
        }

        toml::from_str(&raw).map_err(|_| AuthError::CorruptCredentials)
    }

    fn write(&self, credentials: &Credentials) -> Result<(), AuthError> {
        let parent = self.path.parent().ok_or(AuthError::NoConfigDir)?;
        let existed = parent.exists();
        fs::create_dir_all(parent)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if !existed {
                fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
            }
        }

        let data = toml::to_string_pretty(credentials).map_err(|_| AuthError::Serialize)?;
        let temporary = self.path.with_extension("toml.tmp");
        fs::write(&temporary, data)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
        }

        fs::rename(&temporary, &self.path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))?;
        }

        Ok(())
    }
}
