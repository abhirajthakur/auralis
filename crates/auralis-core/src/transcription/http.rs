//! Shared HTTP plumbing for remote providers: client construction, timeout
//! mapping, and status/error parsing into [`ProviderError`].

use std::time::Duration;

use crate::transcription::ProviderError;

pub const USER_AGENT: &str = concat!("Auralis/", env!("CARGO_PKG_VERSION"));
/// Total request budget. Whisper-large on a 30 s clip is typically < 2 s on
/// Groq; 60 s leaves headroom for slow networks without hanging the UI.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("reqwest client with static config")
}

/// Run a request future, mapping transport errors to the taxonomy.
pub async fn send(
    provider: &'static str,
    fut: impl std::future::Future<Output = Result<reqwest::Response, reqwest::Error>>,
) -> Result<reqwest::Response, ProviderError> {
    let resp = fut.await.map_err(|e| {
        if e.is_timeout() {
            ProviderError::Timeout { provider, secs: REQUEST_TIMEOUT.as_secs() }
        } else if e.is_connect() {
            ProviderError::Network {
                provider,
                detail: "connection failed — check internet or proxy".into(),
            }
        } else {
            ProviderError::Network { provider, detail: e.to_string() }
        }
    })?;
    resp.error_for_status_ref().map_err(|e| {
        let status = e.status().map(|s| s.as_u16()).unwrap_or(0);
        ProviderError::Api {
            provider,
            status,
            message: format!("HTTP {status}"),
        }
    })?;
    Ok(resp)
}

/// Convert a non-2xx response into a rich [`ProviderError`] by reading the
/// provider's documented error body. Accepts the response *after* callers
/// saw a failing status.
pub async fn error_from_response(
    provider: &'static str,
    status: reqwest::StatusCode,
    body: &str,
) -> ProviderError {
    let status_code = status.as_u16();
    let message = extract_error_message(body).unwrap_or_else(|| {
        let head = body.chars().take(180).collect::<String>();
        if head.trim().is_empty() {
            format!("HTTP {status_code}")
        } else {
            head
        }
    });

    match status_code {
        401 | 403 => ProviderError::InvalidKey { provider },
        429 => ProviderError::RateLimited { provider, detail: message },
        408 | 504 => ProviderError::Timeout { provider, secs: REQUEST_TIMEOUT.as_secs() },
        _ => ProviderError::Api { provider, status: status_code, message },
    }
}

/// Best-effort extraction of a human message from common error shapes:
/// OpenAI/Groq `{"error":{"message":...}}` and Gemini `{"error":{"message":...}}`.
fn extract_error_message(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.pointer("/error/message")
        .and_then(|m| m.as_str())
        .map(str::to_string)
        .or_else(|| v.get("message").and_then(|m| m.as_str()).map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn error_parsing_openai_shape() {
        let body = r#"{"error":{"message":"Incorrect API key provided.","type":"invalid_request_error"}}"#;
        let err = error_from_response("openai", reqwest::StatusCode::UNAUTHORIZED, body).await;
        assert!(matches!(err, ProviderError::InvalidKey { .. }));
    }

    #[tokio::test]
    async fn error_parsing_rate_limit() {
        let body = r#"{"error":{"message":"Rate limit reached for model whisper-large-v3"}}"#;
        let err = error_from_response("groq", reqwest::StatusCode::TOO_MANY_REQUESTS, body).await;
        match err {
            ProviderError::RateLimited { detail, .. } => {
                assert!(detail.contains("Rate limit reached"));
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn error_parsing_garbage_body() {
        let err = error_from_response("groq", reqwest::StatusCode::INTERNAL_SERVER_ERROR, "<html>oops").await;
        match err {
            ProviderError::Api { status, message, .. } => {
                assert_eq!(status, 500);
                assert!(message.contains("<html>"));
            }
            other => panic!("expected Api, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn error_parsing_empty_body() {
        let err = error_from_response("gemini", reqwest::StatusCode::BAD_GATEWAY, "").await;
        match err {
            ProviderError::Api { status, message, .. } => {
                assert_eq!(status, 502);
                assert_eq!(message, "HTTP 502");
            }
            other => panic!("expected Api, got {other:?}"),
        }
    }
}
