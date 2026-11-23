//! Anthropic Claude refinement via the Messages API.
//! Docs: https://docs.anthropic.com/en/api/messages

use std::time::Instant;

use async_trait::async_trait;

use crate::refinement::{
    RefinementProvider, RefinementProviderInfo, RefinementRequest, RefinementResult,
    REFINE_PROVIDERS,
};
use crate::security::SecretStore;
use crate::transcription::http;
use crate::transcription::{stored_key, ConnectionTest, ProviderError};

pub const BASE_URL: &str = "https://api.anthropic.com";
pub const API_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    base_url: String,
}

impl AnthropicProvider {
    pub fn new() -> Self {
        Self { base_url: BASE_URL.into() }
    }

    pub fn with_base_url(base_url: String) -> Self {
        Self { base_url }
    }
}

impl Default for AnthropicProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the Messages API body. Pure, unit-testable.
pub fn build_messages_body(
    model: &str,
    system_prompt: &str,
    text: &str,
) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "max_tokens": 4096,
        "temperature": 0.2,
        "system": system_prompt,
        "messages": [
            { "role": "user", "content": text }
        ]
    })
}

/// Extract text blocks from the Messages API response.
pub fn parse_messages_response(body: &str) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("unparseable response: {e}"))?;
    v.get("content")
        .and_then(|c| c.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| {
                    if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                        b.get("text").and_then(|t| t.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("")
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "no text block in response".to_string())
}

#[async_trait]
impl RefinementProvider for AnthropicProvider {
    fn id(&self) -> &'static str {
        "anthropic"
    }

    fn info(&self) -> &'static RefinementProviderInfo {
        &REFINE_PROVIDERS[4]
    }

    async fn test_connection(&self, store: &dyn SecretStore) -> ConnectionTest {
        let started = Instant::now();
        let result = (|| async {
            let key = stored_key(store, "anthropic")?;
            // Tiny real request — /v1/models exists but messages proves auth+model.
            let body = serde_json::json!({
                "model": "claude-3-5-haiku-latest",
                "max_tokens": 8,
                "messages": [{"role": "user", "content": "ping"}]
            });
            let url = format!("{}/v1/messages", self.base_url);
            let fut = http::client()
                .post(&url)
                .header("x-api-key", &key)
                .header("anthropic-version", API_VERSION)
                .json(&body)
                .send();
            let resp = http::send("anthropic", fut).await?;
            let status = resp.status();
            let text = resp
                .text()
                .await
                .map_err(|e| ProviderError::Network { provider: "anthropic", detail: e.to_string() })?;
            if !status.is_success() {
                return Err(http::error_from_response("anthropic", status, &text).await);
            }
            Ok::<(), ProviderError>(())
        })()
        .await;
        match result {
            Ok(()) => ConnectionTest {
                ok: true,
                provider: "anthropic".into(),
                message: "Connection OK — API key accepted.".into(),
                latency: started.elapsed(),
            },
            Err(e) => ConnectionTest {
                ok: false,
                provider: "anthropic".into(),
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
        let key = stored_key(store, "anthropic")?;
        let body = build_messages_body(&request.model, &request.system_prompt, &request.text);
        let url = format!("{}/v1/messages", self.base_url);
        let started = Instant::now();
        let fut = http::client()
            .post(&url)
            .header("x-api-key", &key)
            .header("anthropic-version", API_VERSION)
            .json(&body)
            .send();
        let resp = http::send("anthropic", fut).await?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| ProviderError::Network { provider: "anthropic", detail: e.to_string() })?;
        if !status.is_success() {
            return Err(http::error_from_response("anthropic", status, &text).await);
        }
        let refined = parse_messages_response(&text).map_err(|m| ProviderError::Api {
            provider: "anthropic",
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
    fn messages_body_shape() {
        let body = build_messages_body("claude-3-5-haiku-latest", "SYS", "txt");
        assert_eq!(body["model"], "claude-3-5-haiku-latest");
        assert_eq!(body["system"], "SYS");
        assert_eq!(body["max_tokens"], 4096);
        assert_eq!(body["messages"][0]["role"], "user");
    }

    #[test]
    fn parses_text_blocks() {
        let body = serde_json::json!({
            "content": [
                {"type": "text", "text": "Hello "},
                {"type": "text", "text": "world"},
                {"type": "tool_use", "id": "x"}
            ]
        })
        .to_string();
        assert_eq!(parse_messages_response(&body).unwrap(), "Hello world");
    }

    #[test]
    fn parse_error_when_no_text() {
        assert!(parse_messages_response("{\"content\":[]}").is_err());
    }
}
