use async_trait::async_trait;

use crate::claude::PromptEnvelope;
use crate::error::AgentError;
use crate::model::ModelResult;

/// Abstraction over LLM providers.
///
/// `ClaudeCapability` is the live production implementation.
/// `MockClaudeCapability` is the deterministic demo/test implementation.
/// `OllamaCapability` is the local LLM implementation.
///
/// All agent structs accept `Arc<dyn LlmCapability>` so that tests and demos
/// can inject a mock without requiring a real Claude API key.
///
/// Aligned with VERITAS ModelCapability RFC: every invocation can return
/// structured metadata (confidence, latency, token usage) alongside the text
/// output via `call_with_metadata`. Existing impls only need to implement
/// `call`; the default implementation handles the rest.
#[async_trait]
pub trait LlmCapability: Send + Sync + std::fmt::Debug {
    /// Simple text-in, text-out call (backward-compatible).
    async fn call(&self, prompt: &PromptEnvelope) -> Result<String, AgentError>;

    /// Structured call returning model metadata alongside text output.
    ///
    /// The default implementation wraps `call()` and records wall-clock
    /// latency. Provider-specific implementations (e.g. `ClaudeCapability`)
    /// can override this to surface token usage and confidence scores from
    /// the API response.
    async fn call_with_metadata(&self, prompt: &PromptEnvelope) -> Result<ModelResult, AgentError> {
        let start = std::time::Instant::now();
        let output = self.call(prompt).await?;
        let latency_ms = start.elapsed().as_millis() as u64;
        Ok(ModelResult {
            output,
            confidence: None,
            latency_ms,
            token_usage: None,
            model_id: String::new(),
            model_version: String::new(),
        })
    }
}
