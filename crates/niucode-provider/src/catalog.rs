//! Provider/model capability catalog.
//!
//! This is deliberately static in M5-001: enough to offer safe cross-provider
//! selection without inventing a remote discovery/config system.

use std::fmt;

pub const DEFAULT_CONTEXT_WINDOW: usize = 64_000;
pub const CONTEXT_RESERVE_PERCENT: usize = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub context_window: usize,
    pub max_output: usize,
    pub supports_tools: bool,
    pub supports_reasoning: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub api_key_env: &'static str,
    pub default_base_url: &'static str,
    pub default_model: &'static str,
    pub models: &'static [ModelDescriptor],
}

const DEEPSEEK_MODELS: &[ModelDescriptor] = &[
    ModelDescriptor {
        id: "deepseek-flash",
        label: "DeepSeek Flash",
        context_window: 64_000,
        max_output: 8_000,
        supports_tools: true,
        supports_reasoning: true,
    },
    ModelDescriptor {
        id: "deepseek-pro",
        label: "DeepSeek Pro",
        context_window: 64_000,
        max_output: 8_000,
        supports_tools: true,
        supports_reasoning: true,
    },
    ModelDescriptor {
        id: "deepseek-reasoner",
        label: "DeepSeek Reasoner",
        context_window: 64_000,
        max_output: 8_000,
        supports_tools: true,
        supports_reasoning: true,
    },
];

const OPENAI_MODELS: &[ModelDescriptor] = &[
    ModelDescriptor {
        id: "gpt-4o-mini",
        label: "GPT-4o mini",
        context_window: 128_000,
        max_output: 16_384,
        supports_tools: true,
        supports_reasoning: false,
    },
    ModelDescriptor {
        id: "gpt-4o",
        label: "GPT-4o",
        context_window: 128_000,
        max_output: 16_384,
        supports_tools: true,
        supports_reasoning: false,
    },
    ModelDescriptor {
        id: "o4-mini",
        label: "o4-mini",
        context_window: 200_000,
        max_output: 100_000,
        supports_tools: true,
        supports_reasoning: true,
    },
];

const PROVIDERS: &[ProviderDescriptor] = &[
    ProviderDescriptor {
        id: "deepseek",
        label: "DeepSeek",
        api_key_env: "DEEPSEEK_API_KEY",
        default_base_url: "https://api.deepseek.com",
        default_model: "deepseek-flash",
        models: DEEPSEEK_MODELS,
    },
    ProviderDescriptor {
        id: "openai",
        label: "OpenAI",
        api_key_env: "OPENAI_API_KEY",
        default_base_url: "https://api.openai.com/v1",
        default_model: "gpt-4o-mini",
        models: OPENAI_MODELS,
    },
];

pub fn providers() -> &'static [ProviderDescriptor] {
    PROVIDERS
}

pub fn find_provider(id: &str) -> Option<&'static ProviderDescriptor> {
    PROVIDERS.iter().find(|provider| provider.id == id)
}

pub fn find_model(provider: &str, model: &str) -> Option<&'static ModelDescriptor> {
    find_provider(provider)?
        .models
        .iter()
        .find(|candidate| candidate.id == model)
}

pub fn context_window(provider: &str, model: &str) -> usize {
    find_model(provider, model)
        .map(|candidate| candidate.context_window)
        .unwrap_or(DEFAULT_CONTEXT_WINDOW)
}

pub fn derived_context_budget(provider: &str, model: &str) -> usize {
    let window = context_window(provider, model);
    let reserve = window.saturating_mul(CONTEXT_RESERVE_PERCENT) / 100;
    window.saturating_sub(reserve).max(8_000)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSelection {
    pub provider: String,
    pub model: String,
}

impl ModelSelection {
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
        }
    }

    /// Parse `provider/model` or a bare model resolved against `current`.
    pub fn parse(input: &str, current_provider: &str) -> Result<Self, String> {
        let input = input.trim();
        if input.is_empty() {
            return Err("model name is empty".into());
        }
        if let Some((provider, model)) = input.split_once('/') {
            if provider.is_empty() || model.is_empty() || model.contains('/') {
                return Err(format!("invalid model selection {input:?}"));
            }
            if find_provider(provider).is_none() {
                return Err(format!("unknown provider {provider:?}"));
            }
            return Ok(Self::new(provider, model));
        }
        Ok(Self::new(current_provider, input))
    }

    pub fn canonical(&self) -> String {
        format!("{}/{}", self.provider, self.model)
    }
}

impl fmt::Display for ModelSelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.canonical())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn providers_are_stable_and_have_models() {
        assert_eq!(providers().len(), 2);
        assert_eq!(
            find_provider("deepseek").expect("deepseek").default_model,
            "deepseek-flash"
        );
        assert_eq!(
            find_model("openai", "gpt-4o")
                .expect("gpt-4o")
                .context_window,
            128_000
        );
        assert!(find_model("openai", "missing").is_none());
    }

    #[test]
    fn parses_canonical_and_bare_selections() {
        assert_eq!(
            ModelSelection::parse("openai/gpt-4o", "deepseek").expect("canonical"),
            ModelSelection::new("openai", "gpt-4o")
        );
        assert_eq!(
            ModelSelection::parse("deepseek-pro", "deepseek").expect("bare"),
            ModelSelection::new("deepseek", "deepseek-pro")
        );
        assert!(ModelSelection::parse("missing/gpt-4o", "deepseek").is_err());
        assert!(ModelSelection::parse("openai/gpt-4o/extra", "deepseek").is_err());
    }

    #[test]
    fn unknown_models_get_a_conservative_budget() {
        assert_eq!(
            context_window("deepseek", "unknown"),
            DEFAULT_CONTEXT_WINDOW
        );
        let budget = derived_context_budget("deepseek", "deepseek-flash");
        assert!(budget > 8_000 && budget < 64_000);
    }
}
