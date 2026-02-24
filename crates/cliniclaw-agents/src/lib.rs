pub mod ambient_doc;
pub mod cds;
pub mod claude;
pub mod error;
pub mod llm;
pub mod mock_claude;
pub mod order_entry;
pub mod prior_auth;

pub use ambient_doc::{AmbientDocAgent, AmbientDocInput, AmbientDocOutput};
pub use cds::{CdsCard, CdsIndicator, CdsSuggestion};
pub use claude::{ClaudeCapability, ClaudeResponse, DeidentifiedPrompt, PromptEnvelope};
pub use error::AgentError;
pub use llm::LlmCapability;
pub use mock_claude::MockClaudeCapability;
pub use order_entry::{OrderEntryAgent, OrderEntryInput, OrderEntryOutput};
pub use prior_auth::{PriorAuthAgent, PriorAuthInput, PriorAuthOutput, PriorAuthStatus};

// Re-export kernel types used in agent outputs
pub use cliniclaw_kernel::Confidence;
