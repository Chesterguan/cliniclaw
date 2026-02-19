pub mod ambient_doc;
pub mod claude;
pub mod error;

pub use ambient_doc::{AmbientDocAgent, AmbientDocInput, AmbientDocOutput};
pub use claude::{ClaudeCapability, ClaudeResponse, DeidentifiedPrompt, PromptEnvelope};
pub use error::AgentError;
