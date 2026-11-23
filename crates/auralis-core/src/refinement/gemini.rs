//! Gemini refinement via `generateContent` with `system_instruction`.
//! Docs: https://ai.google.dev/api/generate-content

use std::time::Instant;

use async_trait::async_trait;

use crate::refinement::{
    RefinementProvider, RefinementProviderInfo, RefinementRequest, RefinementResult,
    REFINE_PROVIDERS,
};
use crate::security::SecretStore;
use crate::transcription::http;
use crate::transcription::{stored_key, ConnectionTest, ProviderError};

pub const BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

pub struct GeminiRefinementProvider {
    base_url: String,
}

impl GeminiRefinementProvider {
    pub fn new() -> Self {
        Self { base_url: BASE_URL.into() }
    }

    pub fn with_base_url(base_url: String) -> Self {
        Self { base_url }
    }
}

impl Default for GeminiRefinementProvider {
    fn default() -> Self {
        Self::new()
    }
}

pub fn build_generate_body(system_prompt: &str, text: &str, language: Option<&str>) -> serde_json::Value {
    let mut user = String::from(text);
    if let Some(lang) = language {
        user = format!("{user}\n\nRespond in {lang}.");
    }
    serde_json::json!({
        "system_instruction": { "parts": [{ "text": system_prompt }] },
        "contents": [{ "role": "user", "parts": [{ "text": user }] }],
        "generationConfig": { "temperature": 0.2 }
    })
}

pub fn parse_generate_response(body: &str) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("unparseable response: {e}"))?;
    v.pointer("/candidates/0/content/parts")
        .and_then(|p| p.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<String>()
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "no text in response".to_string())
}

#[async_trait]
impl RefinementProvider for GeminiRefinementProvider {
    fn id(&self) -> &'static str {
        "gemini"
    }

    fn info(&self) -> &'static RefinementProviderInfo {
        &REFINE_PROVIDERS[2]
    }

    async fn test_connection(&self, store: &dyn SecretStore) -> ConnectionTest {
        let started = Instant::now();
        let result = (|| async {
            let key = stored_key(store, "gemini")?;
            let url = format!("{}/models", self.base_url);
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

    async fn refine(
        &self,
        store: &dyn SecretStore,
        request: &RefinementRequest,
    ) -> Result<RefinementResult, ProviderError> {
        const P: &str = "gemini";
        let key = stored_key(store, P)?;
        let body = build_generate_body(
            &request.system_prompt,
            &request.text,
            request.language.as_deref(),
        );
        let url = format!("{}/models/{}:generateContent", self.base_url, request.model);
        let started = Instant::now();
        let fut = http::client().post(&url).header("x-goog-api-key", &key).json(&body).send();
        let resp = http::send(P, fut).await?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| ProviderError::Network { provider: P, detail: e.to_string() })?;
        if !status.is_success() {
            return Err(http::error_from_response(P, status, &text).await);
        }
        let refined = parse_generate_response(&text).map_err(|m| ProviderError::Api {
            provider: P,
            status: status.as_u16(),
            message: m,
        })?;
        Ok(RefinementResult {
            text: refined,
            model: request.model.clone(),
            latency: started.elapsed(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_body_shape() {
        let body = build_generate_body("SYS", "text here", None);
        assert_eq!(body["system_instruction"]["parts"][0]["text"], "SYS");
        assert_eq!(body["contents"][0]["role"], "user");
        assert_eq!(body["contents"][0]["parts"][0]["text"], "text here");
    }

    #[test]
    fn language_appended() {
        let body = build_generate_body("SYS", "text", Some("German"));
        let user = body["contents"][0]["parts"][0]["text"].as_str().unwrap();
        assert!(user.contains("Respond in German."));
    }

    #[test]
    fn parses_response() {
        let body = serde_json::json!({
            "candidates": [{"content": {"parts": [{"text": "Refined!"}]}}]
        })
        .to_string();
        assert_eq!(parse_generate_response(&body).unwrap(), "Refined!");
    }
}
