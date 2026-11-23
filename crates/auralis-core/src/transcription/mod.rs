//! Speech-to-text provider abstraction.
//!
//! Every provider (remote or local) implements [`SpeechProvider`]. The shell
//! calls providers with audio already encoded as WAV (16 kHz mono) by the
//! audio pipeline, so adapters stay thin and focus on their documented APIs.

pub mod gemini;
pub mod groq;
pub mod http;
pub mod openai;

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::security::SecretStore;

/// Everything a provider needs to fulfil one transcription.
#[derive(Debug, Clone)]
pub struct TranscriptionRequest {
    /// Provider API key, fetched from the OS keyring by the shell.
    pub api_key: String,
    /// Model id, e.g. `whisper-large-v3`.
    pub model: String,
    /// BCP-47 hint (`"en"`, `"de"`); `None` lets the provider auto-detect.
    pub language: Option<String>,
    /// Complete WAV file (RIFF container, 16-bit PCM).
    pub audio_wav: Vec<u8>,
}

/// Provider result with measured latency so the UI can show real timings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub text: String,
    pub model: String,
    #[serde(with = "serde_duration_millis")]
    pub latency: Duration,
}

pub(crate) mod serde_duration_millis {
    use super::*;
    use serde::de::Deserializer;
    use serde::ser::Serializer;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(d.as_millis() as u64)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(des: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Duration::from_millis(u64::deserialize(des)?))
    }
}

/// Cross-provider error taxonomy (see product spec §14).
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("no API key configured for {provider}. Add one in Providers settings.")]
    MissingKey { provider: &'static str },
    #[error("{provider} rejected the API key. Check it in Providers settings.")]
    InvalidKey { provider: &'static str },
    #[error("{provider} rate limit reached{detail}. Try again shortly.")]
    RateLimited { provider: &'static str, detail: String },
    #[error("{provider} did not respond within {secs}s. Check your connection or try again.")]
    Timeout { provider: &'static str, secs: u64 },
    #[error("network error talking to {provider}: {detail}")]
    Network {
        provider: &'static str,
        detail: String,
    },
    #[error("{provider} returned an error ({status}): {message}")]
    Api {
        provider: &'static str,
        status: u16,
        message: String,
    },
    #[error("the recorded audio format is not supported by {provider}")]
    UnsupportedFormat { provider: &'static str },
}

impl ProviderError {
    pub fn provider(&self) -> &'static str {
        match self {
            ProviderError::MissingKey { provider }
            | ProviderError::InvalidKey { provider }
            | ProviderError::RateLimited { provider, .. }
            | ProviderError::Timeout { provider, .. }
            | ProviderError::Network { provider, .. }
            | ProviderError::Api { provider, .. }
            | ProviderError::UnsupportedFormat { provider } => provider,
        }
    }
}

/// Static registry data for the Providers page.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    /// True when the provider can be used with no network and no key.
    pub local: bool,
    /// True when a free tier exists (not unlimited — shown in the UI).
    pub free_tier: bool,
    pub requires_key: bool,
    /// Where the user creates an API key.
    pub key_url: &'static str,
    pub models: &'static [&'static str],
    pub default_model: &'static str,
}

pub const SPEECH_PROVIDERS: &[ProviderInfo] = &[
    ProviderInfo {
        id: "groq",
        display_name: "Groq",
        local: false,
        free_tier: true,
        requires_key: true,
        key_url: "https://console.groq.com/keys",
        models: &["whisper-large-v3", "whisper-large-v3-turbo"],
        default_model: "whisper-large-v3",
    },
    ProviderInfo {
        id: "openai",
        display_name: "OpenAI",
        local: false,
        free_tier: false,
        requires_key: true,
        key_url: "https://platform.openai.com/api-keys",
        models: &["gpt-4o-mini-transcribe", "gpt-4o-transcribe", "whisper-1"],
        default_model: "gpt-4o-mini-transcribe",
    },
    ProviderInfo {
        id: "gemini",
        display_name: "Google Gemini",
        local: false,
        free_tier: true,
        requires_key: true,
        key_url: "https://aistudio.google.com/apikey",
        models: &["gemini-2.0-flash"],
        default_model: "gemini-2.0-flash",
    },
    ProviderInfo {
        id: "local-whisper",
        display_name: "Local Whisper",
        local: true,
        free_tier: true,
        requires_key: false,
        key_url: "",
        models: &["tiny", "tiny.en", "base", "base.en", "small", "small.en"],
        default_model: "base.en",
    },
];

pub fn speech_provider_info(id: &str) -> Option<&'static ProviderInfo> {
    SPEECH_PROVIDERS.iter().find(|p| p.id == id)
}

/// Result of a connection test from the Providers page.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTest {
    pub ok: bool,
    pub provider: String,
    pub message: String,
    #[serde(with = "serde_duration_millis", rename = "latencyMs")]
    pub latency: Duration,
}

/// The behaviour every speech provider implements.
#[async_trait]
pub trait SpeechProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn info(&self) -> &'static ProviderInfo;

    /// Validate credentials cheaply (list models / tiny request).
    async fn test_connection(&self, store: &dyn SecretStore) -> ConnectionTest;

    /// Transcribe a complete WAV clip.
    async fn transcribe(
        &self,
        store: &dyn SecretStore,
        request: &TranscriptionRequest,
    ) -> Result<TranscriptionResult, ProviderError>;
}

/// Fetch the stored key or map to a helpful error.
pub fn stored_key(
    store: &dyn SecretStore,
    provider: &'static str,
) -> Result<String, ProviderError> {
    store
        .get(provider)
        .map_err(|e| match e {
            crate::security::SecretError::NotFound(_) => ProviderError::MissingKey { provider },
            crate::security::SecretError::Backend(m) => ProviderError::Api {
                provider,
                status: 0,
                message: format!("keyring unavailable: {m}"),
            },
        })
        .and_then(|k| {
            if k.trim().is_empty() {
                Err(ProviderError::MissingKey { provider })
            } else {
                Ok(k.trim().to_string())
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_expected_providers() {
        let ids: Vec<&str> = SPEECH_PROVIDERS.iter().map(|p| p.id).collect();
        assert_eq!(ids, vec!["groq", "openai", "gemini", "local-whisper"]);
        let local = speech_provider_info("local-whisper").unwrap();
        assert!(local.local && !local.requires_key);
        assert!(speech_provider_info("groq").unwrap().free_tier);
    }

    #[test]
    fn missing_key_maps_to_helpful_error() {
        let store = crate::security::MemorySecretStore::new();
        let err = stored_key(&store, "groq").unwrap_err();
        assert!(err.to_string().contains("Add one in Providers settings"));
    }

    #[test]
    fn empty_key_is_missing_key() {
        let store = crate::security::MemorySecretStore::new();
        store.set("openai", "   ").unwrap();
        assert!(matches!(
            stored_key(&store, "openai"),
            Err(ProviderError::MissingKey { .. })
        ));
    }
}
