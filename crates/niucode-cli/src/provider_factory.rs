//! Runtime provider construction.
//!
//! Credentials are resolved only when a provider is actually built or its
//! availability is queried. The TUI uses this for cross-provider `/model`.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use niucode_auth::AuthStore;
use niucode_provider::{
    OpenAiConfig, OpenAiProvider, Preset, Provider, RecordingProvider, ThinkingMode,
};

pub struct ProviderFactory {
    auth: AuthStore,
    no_thinking: bool,
    record: Option<PathBuf>,
}

impl ProviderFactory {
    pub fn new(no_thinking: bool, record: Option<PathBuf>) -> Result<Self> {
        Ok(Self {
            auth: AuthStore::new().context("initialize auth store")?,
            no_thinking,
            record,
        })
    }

    pub fn preset(provider: &str) -> Result<Preset> {
        Preset::parse(provider).with_context(|| format!("unknown provider {provider:?}"))
    }

    pub fn available(&self, provider: &str) -> Result<bool> {
        let preset = Self::preset(provider)?;
        Ok(self
            .auth
            .status(preset.as_str())
            .context("resolve provider availability")?
            .effective
            .is_some())
    }

    pub fn auth_source(&self, provider: &str) -> Result<Option<String>> {
        let preset = Self::preset(provider)?;
        Ok(self
            .auth
            .resolve(preset.as_str())
            .context("resolve provider credential")?
            .map(|credential| credential.source.to_string()))
    }

    pub fn build(&self, provider: &str, model: &str) -> Result<Arc<dyn Provider>> {
        let preset = Self::preset(provider)?;
        let credential = self.auth.resolve(preset.as_str())?.with_context(|| {
            format!(
                "no credential for {}; run `niucode auth set {}` or set {}",
                preset.as_str(),
                preset.as_str(),
                preset.api_key_env()
            )
        })?;

        let mut config = OpenAiConfig::preset_config(preset)?;
        config.api_key = credential.key.expose().to_string();
        config.model = model.to_string();
        if self.no_thinking && preset == Preset::DeepSeek {
            config.thinking = Some(ThinkingMode::Disabled);
        }
        let provider: Arc<dyn Provider> = Arc::new(OpenAiProvider::for_preset(preset, config)?);
        if let Some(path) = &self.record {
            Ok(Arc::new(RecordingProvider::new(provider, path.clone())))
        } else {
            Ok(provider)
        }
    }

    pub fn default_model(provider: &str) -> Result<String> {
        let preset = Self::preset(provider)?;
        Ok(niucode_provider::find_provider(preset.as_str())
            .map(|descriptor| descriptor.default_model.to_string())
            .unwrap_or_else(|| preset.default_model().to_string()))
    }
}

pub fn available_model_choices(factory: &ProviderFactory) -> Vec<String> {
    let mut choices = Vec::new();
    for provider in niucode_provider::providers() {
        if factory.available(provider.id).unwrap_or(false) {
            choices.extend(
                provider
                    .models
                    .iter()
                    .map(|model| format!("{}/{}", provider.id, model.id)),
            );
        }
    }
    choices
}

pub fn provider_choices() -> Vec<String> {
    niucode_provider::providers()
        .iter()
        .flat_map(|provider| {
            provider
                .models
                .iter()
                .map(move |model| format!("{}/{}", provider.id, model.id))
        })
        .collect()
}
