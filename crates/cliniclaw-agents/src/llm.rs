use async_trait::async_trait;

use crate::claude::PromptEnvelope;
use crate::error::AgentError;

/// Abstraction over LLM providers.
///
/// `ClaudeCapability` is the live production implementation.
/// `MockClaudeCapability` is the deterministic demo/test implementation.
///
/// All agent structs accept `Arc<dyn LlmCapability>` so that tests and demos
/// can inject a mock without requiring a real Claude API key.
#[async_trait]
pub trait LlmCapability: Send + Sync + std::fmt::Debug {
    async fn call(&self, prompt: &PromptEnvelope) -> Result<String, AgentError>;
}
