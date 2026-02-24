use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};

use crate::error::AgentError;
use crate::llm::LlmCapability;

/// Structured prompt envelope for Claude API calls.
///
/// **Important:** This type does NOT perform de-identification. Callers are
/// responsible for ensuring PHI is minimized before constructing the prompt.
/// A real de-identification layer should be added before production use.
#[derive(Debug, Clone)]
pub struct PromptEnvelope {
    system: String,
    user: String,
}

/// Backward-compatible alias for the renamed type.
pub type DeidentifiedPrompt = PromptEnvelope;

impl PromptEnvelope {
    pub fn build(system: impl Into<String>, user: impl Into<String>) -> Self {
        Self {
            system: system.into(),
            user: user.into(),
        }
    }

    pub fn system(&self) -> &str {
        &self.system
    }

    pub fn user(&self) -> &str {
        &self.user
    }
}

/// Capability wrapper for Claude API calls.
/// The ONLY sanctioned path for making Claude API calls.
pub struct ClaudeCapability {
    client: reqwest::Client,
    api_key: SecretString,
    model: String,
    max_tokens: u32,
}

impl std::fmt::Debug for ClaudeCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaudeCapability")
            .field("model", &self.model)
            .field("max_tokens", &self.max_tokens)
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ClaudeResponse {
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
}

impl ClaudeCapability {
    /// Default timeout for Claude API calls (120 seconds).
    const DEFAULT_TIMEOUT_SECS: u64 = 120;

    pub fn new(api_key: SecretString, model: impl Into<String>, max_tokens: u32) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(Self::DEFAULT_TIMEOUT_SECS))
            .build()
            .expect("failed to build HTTP client");
        Self {
            client,
            api_key,
            model: model.into(),
            max_tokens,
        }
    }

    pub async fn call(&self, prompt: &PromptEnvelope) -> Result<String, AgentError> {
        let request_body = serde_json::json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "system": prompt.system(),
            "messages": [
                {
                    "role": "user",
                    "content": prompt.user()
                }
            ]
        });

        tracing::info!(
            model = %self.model,
            max_tokens = self.max_tokens,
            prompt_system_len = prompt.system().len(),
            prompt_user_len = prompt.user().len(),
            "calling Claude API"
        );

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", self.api_key.expose_secret())
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            // Only include status in error — the response body may echo back PHI
            // from the request prompt.
            tracing::error!(status, "Claude API returned non-success status");
            return Err(AgentError::ClaudeApi(format!("HTTP {status}")));
        }

        let claude_response: ClaudeResponse = response.json().await?;

        let text = claude_response
            .content
            .iter()
            .filter_map(|block| block.text.as_deref())
            .collect::<Vec<_>>()
            .join("");

        if text.is_empty() {
            return Err(AgentError::ClaudeApi("empty response from Claude API".into()));
        }

        tracing::info!(
            response_len = text.len(),
            stop_reason = ?claude_response.stop_reason,
            "Claude API response received"
        );

        Ok(text)
    }
}

#[async_trait]
impl LlmCapability for ClaudeCapability {
    async fn call(&self, prompt: &PromptEnvelope) -> Result<String, AgentError> {
        // Delegate to the inherent method
        self.call(prompt).await
    }
}
