//! Google Gemini speech-to-text via `generateContent` with inline audio.
//!
//! Unlike OpenAI/Groq, Gemini has no dedicated transcription endpoint; audio
//! is sent as base64 `inline_data` alongside a text instruction.
//! Docs: https://ai.google.dev/api/generate-content
//!
//! Note: inline payloads are limited to ~20 MB — short dictation clips are
//! far below that, but this provider is not suitable for hour-long uploads.

use std::time::Instant;

use async_trait::async_trait;
use base64::Engine as _;

use crate::security::SecretStore;
use crate::transcription::http;
use crate::transcription::{
    stored_key, ConnectionTest, ProviderError, ProviderInfo, SpeechProvider,
    TranscriptionRequest, TranscriptionResult,
};

pub const BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

pub const INFO: &ProviderInfo = &crate::transcription::SPEECH_PROVIDERS[2];

#[derive(Debug, Clone, Default)]
pub struct GeminiProvider {
    base_url: Option<String>,
}

impl GeminiProvider {
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

/// Build the `generateContent` request body. Pure function so the exact JSON
/// shape is unit-testable without network.
pub fn build_request_body(model: &str, audio_wav: &[u8], language: Option<&str>) -> serde_json::Value {
    let mut instruction =
        "Transcribe this audio recording exactly as spoken. Return only the transcription \
         text with natural punctuation, no commentary."
            .to_string();
    if let Some(lang) = language {
        instruction.push_str(&format!(" The spoken language is {lang}."));
    }
    serde_json::json!({
        "contents": [{
            "role": "user",
            "parts": [
                { "text": instruction },
                {
                    "inline_data": {
                        "mime_type": "audio/wav",
                        "data": base64::engine::general_purpose::STANDARD.encode(audio_wav),
                    }
                }
            ],
        }],
        "generationConfig": {
            "temperature": 0.0,
            // "model" is in the URL path for Gemini, not the body.
        },
        // silence unused-var lint when model is unchanged
        "_model": model,
    })
}

/// Extract the transcript from a `generateContent` response.
pub fn parse_response(body: &str) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("unparseable response: {e}"))?;
    v.pointer("/candidates/0/content/parts")
        .and_then(|parts| parts.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
                .trim()
                .to_string()
        })
        .ok_or_else(|| "no transcript in response".to_string())
}

#[async_trait]
impl SpeechProvider for GeminiProvider {
    fn id(&self) -> &'static str {
        "gemini"
    }

    fn info(&self) -> &'static ProviderInfo {
        INFO
    }

    async fn test_connection(&self, store: &dyn SecretStore) -> ConnectionTest {
        let started = Instant::now();
        let result = (|| async {
            let key = stored_key(store, "gemini")?;
            let url = format!("{}/models", self.base());
            let fut = http::client().get(&url).header("x-goog-api-key", &key).send();
            let resp = http::send("gemini", fut).await?;
            let status = resp.status();
            let body = resp
                .text()
                .await
                .map_err(|e| ProviderError::Network { provider: "gemini", detail: e.to_string() })?;
            if !status.is_success() {
                return Err(http::error_from_response("gemini", status, &body).await);
            }
            Ok::<(), ProviderError>(())
        })()
        .await;
        match result {
            Ok(()) => ConnectionTest {
                ok: true,
                provider: "gemini".into(),
                message: "Connection OK — API key accepted.".into(),
                latency: started.elapsed(),
            },
            Err(e) => ConnectionTest {
                ok: false,
                provider: "gemini".into(),
                message: e.to_string(),
                latency: started.elapsed(),
            },
        }
    }

    async fn transcribe(
        &self,
        store: &dyn SecretStore,
        request: &TranscriptionRequest,
    ) -> Result<TranscriptionResult, ProviderError> {
        const P: &str = "gemini";
        let key = stored_key(store, P)?;
        if request.audio_wav.is_empty() {
            return Err(ProviderError::UnsupportedFormat { provider: P });
        }

        let body = build_request_body(
            &request.model,
            &request.audio_wav,
            request.language.as_deref(),
        );
        let url = format!("{}/models/{}:generateContent", self.base(), request.model);
        let started = Instant::now();
        let fut = http::client()
            .post(&url)
            .header("x-goog-api-key", &key)
            .json(&body)
            .send();
        let resp = http::send(P, fut).await?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| ProviderError::Network { provider: P, detail: e.to_string() })?;
        if !status.is_success() {
            return Err(http::error_from_response(P, status, &text).await);
        }
        let transcript = parse_response(&text).map_err(|m| ProviderError::Api {
            provider: P,
            status: status.as_u16(),
            message: m,
        })?;

        Ok(TranscriptionResult {
            text: transcript,
            model: request.model.clone(),
            latency: started.elapsed(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body_contains_inline_audio() {
        let body = build_request_body("gemini-2.0-flash", b"RIFFxxxx", Some("de"));
        let data = body
            .pointer("/contents/0/parts/1/inline_data/data")
            .unwrap()
            .as_str()
            .unwrap();
        assert_eq!(base64::engine::general_purpose::STANDARD.decode(data).unwrap(), b"RIFFxxxx");
        assert_eq!(
            body.pointer("/contents/0/parts/1/inline_data/mime_type").unwrap(),
            "audio/wav"
        );
        let text = body.pointer("/contents/0/parts/0/text").unwrap().as_str().unwrap();
        assert!(text.contains("de"));
    }

    #[test]
    fn request_body_omits_language_when_auto() {
        let body = build_request_body("gemini-2.0-flash", b"x", None);
        let text = body.pointer("/contents/0/parts/0/text").unwrap().as_str().unwrap();
        assert!(!text.contains("language is"));
    }

    #[test]
    fn parses_single_part_response() {
        let body = serde_json::json!({
            "candidates": [{"content": {"parts": [{"text": "Hello world."}]}}]
        })
        .to_string();
        assert_eq!(parse_response(&body).unwrap(), "Hello world.");
    }

    #[test]
    fn parses_multi_part_response() {
        let body = serde_json::json!({
            "candidates": [{"content": {"parts": [
                {"text": "Hello "}, {"text": "world."}
            ]}}]
        })
        .to_string();
        assert_eq!(parse_response(&body).unwrap(), "Hello world.");
    }

    #[test]
    fn parse_error_on_missing_candidates() {
        assert!(parse_response("{}").is_err());
    }
}
