//! Groq speech-to-text (Whisper models on Groq's LPU inference).
//!
//! Groq's audio API is OpenAI-compatible:
//! `POST https://api.groq.com/openai/v1/audio/transcriptions`.
//! Docs: https://console.groq.com/docs/speech-to-text

use async_trait::async_trait;

use crate::security::SecretStore;
use crate::transcription::openai::transcribe_compat;
use crate::transcription::{
    stored_key, ConnectionTest, ProviderError, ProviderInfo, SpeechProvider,
    TranscriptionRequest, TranscriptionResult,
};

pub const BASE_URL: &str = "https://api.groq.com/openai/v1";

pub const INFO: &ProviderInfo = &crate::transcription::SPEECH_PROVIDERS[0];

#[derive(Debug, Clone, Default)]
pub struct GroqProvider {
    /// Test-only override.
    #[allow(dead_code)]
    base_url: Option<String>,
}

impl GroqProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: String) -> Self {
        Self { base_url: Some(base_url) }
    }

    fn base(&self) -> String {
        self.base_url.clone().unwrap_or_else(|| BASE_URL.into())
    }
}

#[async_trait]
impl SpeechProvider for GroqProvider {
    fn id(&self) -> &'static str {
        "groq"
    }

    fn info(&self) -> &'static ProviderInfo {
        INFO
    }

    async fn test_connection(&self, store: &dyn SecretStore) -> ConnectionTest {
        crate::transcription::openai::test_compat("groq", &self.base(), store).await
    }

    async fn transcribe(
        &self,
        store: &dyn SecretStore,
        request: &TranscriptionRequest,
    ) -> Result<TranscriptionResult, ProviderError> {
        transcribe_compat("groq", &self.base(), store, request).await
    }
}

/// Quick smoke helper used by tests and the shell's diagnostics.
pub async fn has_valid_key(store: &dyn SecretStore) -> bool {
    stored_key(store, "groq").is_ok()
}
