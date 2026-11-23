//! AI text-refinement provider abstraction.
//!
//! Remote refinement uses full-request (non-streaming) chat completion on
//! purpose: Auralis inserts the *complete* refined text at the cursor, so a
//! partial stream cannot be used. Latency is measured and surfaced instead.

pub mod anthropic;
pub mod gemini;
pub mod openai_compat;
pub mod prompts;

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::security::SecretStore;
use crate::transcription::ProviderError;

#[derive(Debug, Clone)]
pub struct RefinementRequest {
    pub api_key: String,
    pub model: String,
    /// Composed system prompt (engine identity + style + rules).
    pub system_prompt: String,
    /// Raw transcript to refine.
    pub text: String,
    /// Optional language hint ("Respond in English." etc).
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefinementResult {
    pub text: String,
    pub model: String,
    #[serde(with = "crate::transcription::serde_duration_millis")]
    pub latency: Duration,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefinementProviderInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    pub free_tier: bool,
    pub requires_key: bool,
    pub key_url: &'static str,
    pub models: &'static [&'static str],
    pub default_model: &'static str,
    /// OpenAI-style endpoint means it also honours custom base URLs.
    pub openai_compatible: bool,
}

pub const REFINE_PROVIDERS: &[RefinementProviderInfo] = &[
    RefinementProviderInfo {
        id: "groq",
        display_name: "Groq",
        free_tier: true,
        requires_key: true,
        key_url: "https://console.groq.com/keys",
        models: &["llama-3.3-70b-versatile", "llama-3.1-8b-instant"],
        default_model: "llama-3.3-70b-versatile",
        openai_compatible: true,
    },
    RefinementProviderInfo {
        id: "openrouter",
        display_name: "OpenRouter",
        free_tier: true,
        requires_key: true,
        key_url: "https://openrouter.ai/keys",
        models: &["openai/gpt-4o-mini", "google/gemini-2.0-flash-001", "meta-llama/llama-3.3-70b-instruct"],
        default_model: "openai/gpt-4o-mini",
        openai_compatible: true,
    },
    RefinementProviderInfo {
        id: "gemini",
        display_name: "Google Gemini",
        free_tier: true,
        requires_key: true,
        key_url: "https://aistudio.google.com/apikey",
        models: &["gemini-2.0-flash", "gemini-2.0-flash-lite"],
        default_model: "gemini-2.0-flash",
        openai_compatible: false,
    },
    RefinementProviderInfo {
        id: "openai",
        display_name: "OpenAI",
        free_tier: false,
        requires_key: true,
        key_url: "https://platform.openai.com/api-keys",
        models: &["gpt-4o-mini", "gpt-4o", "gpt-4.1-mini"],
        default_model: "gpt-4o-mini",
        openai_compatible: true,
    },
    RefinementProviderInfo {
        id: "anthropic",
        display_name: "Anthropic Claude",
        free_tier: false,
        requires_key: true,
        key_url: "https://console.anthropic.com/settings/keys",
        models: &["claude-3-5-haiku-latest", "claude-3-5-sonnet-latest"],
        default_model: "claude-3-5-haiku-latest",
        openai_compatible: false,
    },
    RefinementProviderInfo {
        id: "openai-compatible",
        display_name: "Custom (OpenAI-compatible)",
        free_tier: false,
        requires_key: true,
        key_url: "",
        models: &[],
        default_model: "",
        openai_compatible: true,
    },
];

pub fn refinement_provider_info(id: &str) -> Option<&'static RefinementProviderInfo> {
    REFINE_PROVIDERS.iter().find(|p| p.id == id)
}

#[async_trait]
pub trait RefinementProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn info(&self) -> &'static RefinementProviderInfo;

    async fn test_connection(&self, store: &dyn SecretStore) -> crate::transcription::ConnectionTest;

    async fn refine(
        &self,
        store: &dyn SecretStore,
        request: &RefinementRequest,
    ) -> Result<RefinementResult, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_all_required_providers() {
        let ids: Vec<&str> = REFINE_PROVIDERS.iter().map(|p| p.id).collect();
        for required in ["groq", "openrouter", "gemini", "openai", "anthropic", "openai-compatible"] {
            assert!(ids.contains(&required), "missing {required}");
        }
        assert!(refinement_provider_info("groq").unwrap().free_tier);
        assert!(!refinement_provider_info("openai").unwrap().free_tier);
        assert!(!refinement_provider_info("openai-compatible").unwrap().free_tier);
    }
}
