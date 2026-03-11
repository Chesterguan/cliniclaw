pub mod ambient_doc;
pub mod cds;
pub mod claude;
pub mod discharge_plan;
pub mod error;
pub mod lab_review;
pub mod llm;
pub mod mock_claude;
pub mod nurse_assess;
pub mod ollama;
pub mod order_entry;
pub mod pharmacy_review;
pub mod prior_auth;
pub mod triage_assess;

pub use ambient_doc::{AmbientDocAgent, AmbientDocInput, AmbientDocOutput};
pub use cds::{CdsCard, CdsIndicator, CdsSuggestion};
pub use claude::{ClaudeCapability, ClaudeResponse, DeidentifiedPrompt, PromptEnvelope};
pub use discharge_plan::{DischargePlanAgent, DischargePlanInput, DischargePlanOutput};
pub use error::AgentError;
pub use lab_review::{LabReviewAgent, LabReviewInput, LabReviewOutput};
pub use llm::LlmCapability;
pub use mock_claude::MockClaudeCapability;
pub use nurse_assess::{NurseAssessAgent, NurseAssessInput, NurseAssessOutput};
pub use ollama::OllamaCapability;
pub use order_entry::{OrderEntryAgent, OrderEntryInput, OrderEntryOutput};
pub use pharmacy_review::{PharmacyReviewAgent, PharmacyReviewInput, PharmacyReviewOutput};
pub use prior_auth::{PriorAuthAgent, PriorAuthInput, PriorAuthOutput, PriorAuthStatus};
pub use triage_assess::{TriageAssessAgent, TriageAssessInput, TriageAssessOutput};

// Re-export kernel types used in agent outputs
pub use cliniclaw_kernel::Confidence;
