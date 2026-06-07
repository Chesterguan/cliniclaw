//! Minimal remote-LLM adapters (Claude + DeepSeek) implementing `LlmCapability`,
//! so the long-horizon experiment can use frontier models as a "ceiling" arm.
//!
//! Request shapes are deliberately clean — model + max_tokens + messages only,
//! no temperature / top_p / budget_tokens — so they are safe on Opus 4.8 (which
//! rejects those) and on any OpenAI-compatible endpoint. API keys come from the
//! environment; they are never logged.

use async_trait::async_trait;

use cliniclaw_agents::{LlmCapability, PromptEnvelope};
use cliniclaw_agents::AgentError;

/// Strip a leading ```json / ``` fence if a model wrapped its JSON.
fn strip_fences(text: &str) -> String {
    let t = text.trim();
    if let Some(rest) = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")) {
        if let Some(inner) = rest.strip_suffix("```") {
            return inner.trim().to_string();
        }
    }
    t.to_string()
}

// ── Claude (Anthropic Messages API) ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ClaudeRemote {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl ClaudeRemote {
    pub fn new(api_key: String, model: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("build reqwest client");
        Self { client, api_key, model: model.into() }
    }
}

#[async_trait]
impl LlmCapability for ClaudeRemote {
    async fn call(&self, prompt: &PromptEnvelope) -> Result<String, AgentError> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 2048,
            "system": prompt.system(),
            "messages": [{ "role": "user", "content": prompt.user() }],
        });
        let resp = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::ClaudeApi(format!("request failed: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            return Err(AgentError::ClaudeApi(format!("Anthropic HTTP {status}")));
        }
        let v: serde_json::Value = resp.json().await
            .map_err(|e| AgentError::ClaudeApi(format!("parse failed: {e}")))?;
        // First text block in content[].
        let text = v.get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.iter().find(|b| b.get("type").and_then(|t| t.as_str()) == Some("text")))
            .and_then(|b| b.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        if text.is_empty() {
            return Err(AgentError::ClaudeApi("empty Anthropic response".into()));
        }
        Ok(strip_fences(text))
    }
}

// ── DeepSeek (OpenAI-compatible chat completions) ────────────────────────────

#[derive(Debug, Clone)]
pub struct DeepSeekRemote {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl DeepSeekRemote {
    pub fn new(api_key: String, model: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .expect("build reqwest client");
        Self { client, api_key, model: model.into() }
    }
}

#[async_trait]
impl LlmCapability for DeepSeekRemote {
    async fn call(&self, prompt: &PromptEnvelope) -> Result<String, AgentError> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": prompt.system() },
                { "role": "user", "content": prompt.user() },
            ],
            "stream": false,
        });
        let resp = self.client
            .post("https://api.deepseek.com/chat/completions")
            .header("authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::ClaudeApi(format!("request failed: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            return Err(AgentError::ClaudeApi(format!("DeepSeek HTTP {status}")));
        }
        let v: serde_json::Value = resp.json().await
            .map_err(|e| AgentError::ClaudeApi(format!("parse failed: {e}")))?;
        let text = v.get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        if text.is_empty() {
            return Err(AgentError::ClaudeApi("empty DeepSeek response".into()));
        }
        Ok(strip_fences(text))
    }
}
