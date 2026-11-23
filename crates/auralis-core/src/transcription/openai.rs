//! OpenAI transcription (also serves as the base for Groq, whose audio API
//! is OpenAI-compatible).
//!
//! API: `POST {base}/audio/transcriptions`, multipart form with `file`,
//! `model`, optional `language`; response `{"text": "..."}`.
//! Docs: https://platform.openai.com/docs/api-reference/audio/createTranscription

use std::time::Instant;

use async_trait::async_trait;

use crate::security::SecretStore;
use crate::transcription::http;
use crate::transcription::{
    stored_key, ConnectionTest, ProviderError, ProviderInfo, SpeechProvider, TranscriptionRequest,
    TranscriptionResult,
};

pub const INFO: &ProviderInfo = {
    // mirror of the registry entry, used by the adapter directly
    &crate::transcription::SPEECH_PROVIDERS[1]
};

#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    base_url: String,
}

impl OpenAiProvider {
    pub fn new() -> Self {
        Self { base_url: "https://api.openai.com/v1".into() }
    }

    /// Test-only: point at a local mock server.
    pub fn with_base_url(base_url: String) -> Self {
        Self { base_url }
    }
}

impl Default for OpenAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared implementation for OpenAI-compatible transcription endpoints
/// (OpenAI itself and Groq).
pub(crate) async fn transcribe_compat(
    provider: &'static str,
    base_url: &str,
    store: &dyn SecretStore,
    request: &TranscriptionRequest,
) -> Result<TranscriptionResult, ProviderError> {
    let key = stored_key(store, provider)?;
    if request.audio_wav.is_empty() {
        return Err(ProviderError::UnsupportedFormat { provider });
    }

    let form = reqwest::multipart::Form::new()
        .text("model", request.model.clone())
        .text("response_format", "json")
        .part(
            "file",
            reqwest::multipart::Part::bytes(request.audio_wav.clone())
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .map_err(|e| ProviderError::Network { provider, detail: e.to_string() })?,
        );
    let form = match &request.language {
        Some(lang) => form.text("language", lang.clone()),
        None => form,
    };

    let url = format!("{base_url}/audio/transcriptions");
    let client = http::client();
    let started = Instant::now();
    let fut = client
        .post(&url)
        .bearer_auth(&key)
        .multipart(form)
        .send();
    let resp = http::send(provider, fut).await?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| ProviderError::Network {
        provider,
        detail: e.to_string(),
    })?;
    if !status.is_success() {
        return Err(http::error_from_response(provider, status, &body).await);
    }

    let parsed: serde_json::Value = serde_json::from_str(&body).map_err(|e| ProviderError::Api {
        provider,
        status: status.as_u16(),
        message: format!("unparseable response: {e}"),
    })?;
    let text = parsed
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();

    Ok(TranscriptionResult {
        text,
        model: request.model.clone(),
        latency: started.elapsed(),
    })
}

/// Shared connection test: `GET {base}/models`.
pub(crate) async fn test_compat(
    provider: &'static str,
    base_url: &str,
    store: &dyn SecretStore,
) -> ConnectionTest {
    let started = Instant::now();
    let result = (|| async {
        let key = stored_key(store, provider)?;
        let url = format!("{base_url}/models");
        let fut = http::client().get(&url).bearer_auth(&key).send();
        let resp = http::send(provider, fut).await?;
        let status = resp.status();
        let body = resp.text().await.map_err(|e| ProviderError::Network {
            provider,
            detail: e.to_string(),
        })?;
        if !status.is_success() {
            return Err(http::error_from_response(provider, status, &body).await);
        }
        Ok::<(), ProviderError>(())
    })()
    .await;

    match result {
        Ok(()) => ConnectionTest {
            ok: true,
            provider: provider.into(),
            message: "Connection OK — API key accepted.".into(),
            latency: started.elapsed(),
        },
        Err(e) => ConnectionTest {
            ok: false,
            provider: provider.into(),
            message: e.to_string(),
            latency: started.elapsed(),
        },
    }
}

#[async_trait]
impl SpeechProvider for OpenAiProvider {
    fn id(&self) -> &'static str {
        "openai"
    }

    fn info(&self) -> &'static ProviderInfo {
        INFO
    }

    async fn test_connection(&self, store: &dyn SecretStore) -> ConnectionTest {
        test_compat("openai", &self.base_url, store).await
    }

    async fn transcribe(
        &self,
        store: &dyn SecretStore,
        request: &TranscriptionRequest,
    ) -> Result<TranscriptionResult, ProviderError> {
        transcribe_compat("openai", &self.base_url, store, request).await
    }
}
