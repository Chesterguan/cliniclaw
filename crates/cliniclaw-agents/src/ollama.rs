use async_trait::async_trait;

use crate::claude::PromptEnvelope;
use crate::error::AgentError;
use crate::llm::LlmCapability;

/// LLM capability backed by a local Ollama instance.
///
/// Calls `POST http://{host}/api/chat` with the model specified at construction.
/// Ollama must be running locally (`ollama serve`).
#[derive(Debug, Clone)]
pub struct OllamaCapability {
    client: reqwest::Client,
    base_url: String,
    model: String,
}

#[derive(Debug, serde::Deserialize)]
struct OllamaChatResponse {
    message: OllamaChatMessage,
}

#[derive(Debug, serde::Deserialize)]
struct OllamaChatMessage {
    content: String,
}

impl OllamaCapability {
    /// Default timeout for Ollama calls (5 minutes — local models can be slow).
    const DEFAULT_TIMEOUT_SECS: u64 = 300;

    pub fn new(model: impl Into<String>) -> Self {
        Self::with_base_url("http://localhost:11434", model)
    }

    pub fn with_base_url(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(Self::DEFAULT_TIMEOUT_SECS))
            .build()
            .expect("failed to build reqwest client");
        Self {
            client,
            base_url: base_url.into(),
            model: model.into(),
        }
    }
}

#[async_trait]
impl LlmCapability for OllamaCapability {
    async fn call(&self, prompt: &PromptEnvelope) -> Result<String, AgentError> {
        let url = format!("{}/api/chat", self.base_url);

        let request_body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": prompt.system()},
                {"role": "user", "content": prompt.user()}
            ],
            "stream": false,
            "options": {
                "temperature": 0.3,
                "num_predict": 4096
            }
        });

        tracing::info!(
            model = %self.model,
            base_url = %self.base_url,
            system_len = prompt.system().len(),
            user_len = prompt.user().len(),
            "calling Ollama"
        );

        let response = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AgentError::ClaudeApi(format!("Ollama request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            tracing::error!(status, body_len = body.len(), "Ollama returned error");
            return Err(AgentError::ClaudeApi(format!("Ollama HTTP {status}")));
        }

        let chat_response: OllamaChatResponse = response
            .json()
            .await
            .map_err(|e| AgentError::ClaudeApi(format!("Ollama response parse error: {e}")))?;

        let text = chat_response.message.content;

        if text.is_empty() {
            return Err(AgentError::ClaudeApi("empty response from Ollama".into()));
        }

        tracing::info!(
            response_len = text.len(),
            "Ollama response received"
        );

        // Ollama models sometimes wrap JSON in markdown code fences — strip them.
        Ok(strip_code_fences(&text))
    }
}

/// Strip markdown code fences that local models often add around JSON output.
fn strip_code_fences(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        if let Some(inner) = rest.strip_suffix("```") {
            return inner.trim().to_string();
        }
    }
    if let Some(rest) = trimmed.strip_prefix("```") {
        if let Some(inner) = rest.strip_suffix("```") {
            return inner.trim().to_string();
        }
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_code_fences_json() {
        let input = "```json\n{\"key\": \"value\"}\n```";
        assert_eq!(strip_code_fences(input), "{\"key\": \"value\"}");
    }

    #[test]
    fn test_strip_code_fences_plain() {
        let input = "```\n{\"key\": \"value\"}\n```";
        assert_eq!(strip_code_fences(input), "{\"key\": \"value\"}");
    }

    #[test]
    fn test_strip_code_fences_none() {
        let input = "{\"key\": \"value\"}";
        assert_eq!(strip_code_fences(input), "{\"key\": \"value\"}");
    }

    #[test]
    fn test_ollama_capability_debug() {
        let cap = OllamaCapability::new("mistral-small");
        let debug = format!("{:?}", cap);
        assert!(debug.contains("mistral-small"));
    }
}
