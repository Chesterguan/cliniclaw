use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use tokio::sync::Semaphore;

use cliniclaw_persist::AuditEvent;
use cliniclaw_policy::{ActionContext, PolicyDecision, PolicyEngine, SkillEvaluation};

use crate::error::AgentError;
use crate::llm::LlmCapability;
use crate::model::{ModelResult, TokenUsage};
use crate::PromptEnvelope;

// ── Pipeline Context ──────────────────────────────────────────────

/// Shared context threaded through the middleware pipeline.
/// Carries encounter identity, policy engine, and accumulated metadata.
#[derive(Debug)]
pub struct PipelineContext {
    pub encounter_id: String,
    pub patient_id: String,
    pub practitioner_id: String,
    pub action: String,
    pub start_time: Instant,
    /// Set by policy middleware after evaluation
    pub skill_eval: Option<SkillEvaluation>,
    /// Set by LLM middleware after call
    pub model_result: Option<ModelResult>,
    /// Held by RateLimitMiddleware — released on complete or error.
    /// Stored per-context (not per-middleware) so concurrent pipeline calls
    /// each hold their own permit.
    pub(crate) rate_limit_permit: Option<tokio::sync::OwnedSemaphorePermit>,
}

impl PipelineContext {
    pub fn new(
        encounter_id: impl Into<String>,
        patient_id: impl Into<String>,
        practitioner_id: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            encounter_id: encounter_id.into(),
            patient_id: patient_id.into(),
            practitioner_id: practitioner_id.into(),
            action: action.into(),
            start_time: Instant::now(),
            skill_eval: None,
            model_result: None,
            rate_limit_permit: None,
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}

// ── Pipeline Result ──────────────────────────────────────────────

/// The result of running an agent through the pipeline.
/// Wraps the agent's typed output with pipeline metadata.
///
/// Callers that need a structured result (audit event, token usage, etc.)
/// build this from the raw LLM response returned by `AgentPipeline::execute`.
#[derive(Debug)]
pub struct PipelineResult<T> {
    pub output: T,
    pub policy_decision: PolicyDecision,
    pub spec_hash: Option<String>,
    pub token_usage: Option<TokenUsage>,
    pub latency_ms: u64,
    pub audit_event: AuditEvent,
}

// ── Middleware Trait ──────────────────────────────────────────────

/// Composable middleware hook points aligned with the VERITAS execution model.
///
/// ```text
/// before_policy → [Policy Gate] → before_llm → [LLM Call] → after_llm → [Verify] → on_complete
///                                                                                    on_error
/// ```
///
/// Default implementations are no-ops. Middleware only overrides what it needs.
#[async_trait]
pub trait AgentMiddleware: Send + Sync {
    /// Called before policy evaluation. Can inspect/modify the ActionContext.
    async fn before_policy(
        &self,
        _ctx: &mut PipelineContext,
        _action_ctx: &mut ActionContext,
    ) -> Result<(), AgentError> {
        Ok(())
    }

    /// Called after policy allows but before the LLM call. Can inspect/modify the prompt.
    async fn before_llm(
        &self,
        _ctx: &mut PipelineContext,
        _prompt: &mut PromptEnvelope,
    ) -> Result<(), AgentError> {
        Ok(())
    }

    /// Called after LLM returns successfully. Can inspect the response.
    async fn after_llm(
        &self,
        _ctx: &mut PipelineContext,
        _response: &str,
    ) -> Result<(), AgentError> {
        Ok(())
    }

    /// Called when the pipeline completes successfully.
    async fn on_complete(&self, _ctx: &mut PipelineContext) {}

    /// Called when any pipeline stage fails. Must not panic.
    async fn on_error(&self, _ctx: &mut PipelineContext, _err: &AgentError) {}
}

// ── Pipeline ─────────────────────────────────────────────────────

/// The Agent Pipeline enforces the VERITAS execution model structurally:
/// Policy → Capability → Agent → Verify → Audit.
///
/// Cross-cutting concerns (rate limiting, token budgets, PHI detection) are
/// composed as middleware rather than duplicated across 8 agent files.
pub struct AgentPipeline {
    middleware: Vec<Arc<dyn AgentMiddleware>>,
    llm: Arc<dyn LlmCapability>,
}

impl AgentPipeline {
    pub fn new(llm: Arc<dyn LlmCapability>) -> Self {
        Self {
            middleware: Vec::new(),
            llm,
        }
    }

    /// Add middleware to the pipeline. Order matters — middleware runs in insertion order.
    pub fn with_middleware(mut self, mw: Arc<dyn AgentMiddleware>) -> Self {
        self.middleware.push(mw);
        self
    }

    /// Execute the VERITAS pipeline for a given agent action.
    ///
    /// The caller provides:
    /// - `ctx`: pipeline context (encounter, patient, practitioner, action)
    /// - `action_ctx`: policy evaluation context (capabilities, properties)
    /// - `policy_engine`: the loaded policy engine
    /// - `prompt`: the LLM prompt envelope
    ///
    /// Returns the raw LLM response text and populated pipeline context.
    /// The caller (agent) is responsible for parsing, building FHIR resources,
    /// and verification — those are domain-specific, not cross-cutting.
    pub async fn execute(
        &self,
        ctx: &mut PipelineContext,
        action_ctx: &mut ActionContext,
        policy_engine: &PolicyEngine,
        prompt: &mut PromptEnvelope,
    ) -> Result<String, AgentError> {
        // ── Stage 1: before_policy middleware ──
        for mw in &self.middleware {
            if let Err(e) = mw.before_policy(&mut *ctx, action_ctx).await {
                self.notify_error(&mut *ctx, &e).await;
                return Err(e);
            }
        }

        // ── Stage 2: Policy evaluation (VERITAS gate) ──
        let skill_eval = policy_engine.evaluate_with_skill(action_ctx)?;

        match &skill_eval.decision {
            PolicyDecision::Deny => {
                tracing::warn!(
                    actor_id = %ctx.practitioner_id,
                    encounter_id = %ctx.encounter_id,
                    action = %ctx.action,
                    "pipeline: policy denied"
                );
                let err = AgentError::PolicyDenied(format!(
                    "{} denied for encounter {}",
                    ctx.action, ctx.encounter_id
                ));
                self.notify_error(&mut *ctx, &err).await;
                return Err(err);
            }
            PolicyDecision::RequireApproval => {
                let err = AgentError::RequiresApproval {
                    action: ctx.action.clone(),
                };
                self.notify_error(&mut *ctx, &err).await;
                return Err(err);
            }
            PolicyDecision::Allow => {
                tracing::info!(
                    actor_id = %ctx.practitioner_id,
                    encounter_id = %ctx.encounter_id,
                    action = %ctx.action,
                    spec_hash = ?skill_eval.spec_hash,
                    "pipeline: policy allowed"
                );
            }
        }
        ctx.skill_eval = Some(skill_eval);

        // ── Stage 3: before_llm middleware ──
        for mw in &self.middleware {
            if let Err(e) = mw.before_llm(&mut *ctx, prompt).await {
                self.notify_error(&mut *ctx, &e).await;
                return Err(e);
            }
        }

        // ── Stage 4: LLM call ──
        let model_result = self.llm.call_with_metadata(prompt).await?;
        let response_text = model_result.output.clone();
        ctx.model_result = Some(model_result);

        // ── Stage 5: after_llm middleware ──
        for mw in &self.middleware {
            if let Err(e) = mw.after_llm(&mut *ctx, &response_text).await {
                self.notify_error(&mut *ctx, &e).await;
                return Err(e);
            }
        }

        // ── Stage 6: on_complete middleware ──
        for mw in &self.middleware {
            mw.on_complete(&mut *ctx).await;
        }

        Ok(response_text)
    }

    /// Notify all middleware of an error.
    async fn notify_error(&self, ctx: &mut PipelineContext, err: &AgentError) {
        for mw in &self.middleware {
            mw.on_error(&mut *ctx, err).await;
        }
    }
}

impl std::fmt::Debug for AgentPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentPipeline")
            .field("middleware_count", &self.middleware.len())
            .finish()
    }
}

// ── Concrete Middleware: Rate Limiter ─────────────────────────────

/// Limits concurrent LLM calls across all agents.
/// Uses a tokio Semaphore — if all permits are taken, the call waits.
///
/// The permit is stored on `PipelineContext` (per-call), not on the middleware
/// instance (per-pipeline). This ensures concurrent pipeline executions each
/// hold their own independent permit.
pub struct RateLimitMiddleware {
    semaphore: Arc<Semaphore>,
}

impl RateLimitMiddleware {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }
}

#[async_trait]
impl AgentMiddleware for RateLimitMiddleware {
    async fn before_llm(
        &self,
        ctx: &mut PipelineContext,
        _prompt: &mut PromptEnvelope,
    ) -> Result<(), AgentError> {
        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| AgentError::ClaudeApi("rate limiter semaphore closed".into()))?;
        tracing::debug!(
            encounter_id = %ctx.encounter_id,
            action = %ctx.action,
            "rate limit permit acquired"
        );
        // Store permit on context — released when context is dropped or on_complete
        ctx.rate_limit_permit = Some(permit);
        Ok(())
    }

    async fn on_complete(&self, ctx: &mut PipelineContext) {
        // Release permit
        ctx.rate_limit_permit = None;
    }

    async fn on_error(&self, ctx: &mut PipelineContext, _err: &AgentError) {
        // Release permit on error too
        ctx.rate_limit_permit = None;
    }
}

// ── Concrete Middleware: Token Budget ─────────────────────────────

/// Tracks token usage per encounter and rejects calls that would exceed budget.
/// Prevents runaway simulations from exhausting Claude API quota.
pub struct TokenBudgetMiddleware {
    max_tokens_per_encounter: u64,
    usage: std::sync::Mutex<std::collections::HashMap<String, u64>>,
}

impl TokenBudgetMiddleware {
    pub fn new(max_tokens_per_encounter: u64) -> Self {
        Self {
            max_tokens_per_encounter,
            usage: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Get current token usage for an encounter.
    pub fn usage_for(&self, encounter_id: &str) -> u64 {
        self.usage
            .lock()
            .map(|u| *u.get(encounter_id).unwrap_or(&0))
            .unwrap_or(0)
    }
}

#[async_trait]
impl AgentMiddleware for TokenBudgetMiddleware {
    async fn before_llm(
        &self,
        ctx: &mut PipelineContext,
        _prompt: &mut PromptEnvelope,
    ) -> Result<(), AgentError> {
        let current = self.usage_for(&ctx.encounter_id);
        if current >= self.max_tokens_per_encounter {
            tracing::warn!(
                encounter_id = %ctx.encounter_id,
                current_tokens = current,
                max_tokens = self.max_tokens_per_encounter,
                "token budget exceeded"
            );
            return Err(AgentError::ClaudeApi(format!(
                "token budget exceeded for encounter {}: {current}/{} tokens",
                ctx.encounter_id, self.max_tokens_per_encounter
            )));
        }
        Ok(())
    }

    async fn after_llm(
        &self,
        ctx: &mut PipelineContext,
        _response: &str,
    ) -> Result<(), AgentError> {
        // Record token usage from the model result
        if let Some(ref model_result) = ctx.model_result {
            if let Some(ref usage) = model_result.token_usage {
                let total = usage.input_tokens + usage.output_tokens;
                if let Ok(mut map) = self.usage.lock() {
                    *map.entry(ctx.encounter_id.clone()).or_insert(0) += total;
                }
            }
        }
        Ok(())
    }
}

// ── Concrete Middleware: PHI Audit ────────────────────────────────

/// Detects potential PHI patterns in LLM prompts and logs warnings.
/// This is a detection layer, not a filter — de-identification is the caller's responsibility.
/// Aligned with CLAUDE.md: "PHI must never appear in log output."
pub struct PhiAuditMiddleware {
    /// Patterns that suggest PHI. Tuples are (category, literal substring to match).
    /// The SSN check is handled separately with digit-position logic below.
    patterns: Vec<(&'static str, &'static str)>,
}

impl PhiAuditMiddleware {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                ("SSN", "\\d{3}-\\d{2}-\\d{4}"), // Not used as regex; see check below
                ("DOB", "date of birth"),
                ("DOB", "DOB:"),
                ("DOB", "Date of Birth"),
                ("MRN", "MRN:"),
                ("MRN", "medical record number"),
            ],
        }
    }

    fn check_for_phi(&self, text: &str) -> Vec<String> {
        let mut findings = Vec::new();

        // Check for SSN-like patterns (###-##-####)
        let chars: Vec<char> = text.chars().collect();
        for i in 0..chars.len().saturating_sub(11) {
            if chars[i].is_ascii_digit()
                && chars.get(i + 3) == Some(&'-')
                && chars.get(i + 6) == Some(&'-')
                && chars[i + 1].is_ascii_digit()
                && chars[i + 2].is_ascii_digit()
                && chars[i + 4].is_ascii_digit()
                && chars[i + 5].is_ascii_digit()
                && chars[i + 7].is_ascii_digit()
                && chars[i + 8].is_ascii_digit()
                && chars[i + 9].is_ascii_digit()
                && chars[i + 10].is_ascii_digit()
            {
                findings.push("SSN-like pattern detected".to_string());
                break; // One finding is enough
            }
        }

        // Check for text-based PHI indicators
        for (category, pattern) in &self.patterns {
            if pattern.contains("\\d") {
                continue; // Skip regex-like patterns (handled above)
            }
            if text.contains(pattern) {
                findings.push(format!("{category} indicator detected"));
            }
        }

        findings
    }
}

impl Default for PhiAuditMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentMiddleware for PhiAuditMiddleware {
    async fn before_llm(
        &self,
        ctx: &mut PipelineContext,
        prompt: &mut PromptEnvelope,
    ) -> Result<(), AgentError> {
        let mut findings = self.check_for_phi(prompt.user());
        findings.extend(self.check_for_phi(prompt.system()));
        if !findings.is_empty() {
            // Log encounter_id ONLY — never the PHI itself
            tracing::warn!(
                encounter_id = %ctx.encounter_id,
                action = %ctx.action,
                finding_count = findings.len(),
                "potential PHI detected in prompt — review de-identification"
            );
        }
        Ok(())
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MockClaudeCapability;
    use cliniclaw_policy::ActionContext;

    // ── Helper: build a test pipeline with mock LLM ──

    fn mock_pipeline() -> AgentPipeline {
        AgentPipeline::new(Arc::new(MockClaudeCapability::new()))
    }

    fn test_ctx() -> PipelineContext {
        PipelineContext::new(
            "enc-001",
            "patient-001",
            "practitioner-001",
            "triage_assess.evaluate",
        )
    }

    fn test_action_ctx() -> ActionContext {
        let mut ctx = ActionContext::new("triage_assess.evaluate", "practitioner-001");
        ctx.capabilities = vec!["triage_assess".to_string()];
        ctx.properties
            .insert("encounter_status".to_string(), "in-progress".to_string());
        ctx.properties
            .insert("encounter.status".to_string(), "in-progress".to_string());
        ctx.properties
            .insert("patient.active".to_string(), "true".to_string());
        ctx
    }

    fn test_policy_engine() -> PolicyEngine {
        let mut engine = PolicyEngine::new();
        engine
            .load_rego_str(
                "triage_assess.rego",
                r#"
package cliniclaw.triage_assess

default decision := "deny"

decision := "allow" if {
    input.action == "triage_assess.evaluate"
    "triage_assess" in input.capabilities
    input.properties.encounter_status == "in-progress"
}
"#,
            )
            .expect("valid rego");
        engine
    }

    fn deny_policy_engine() -> PolicyEngine {
        let mut engine = PolicyEngine::new();
        engine
            .load_rego_str(
                "deny.rego",
                r#"
package cliniclaw.triage_assess

default decision := "deny"
"#,
            )
            .expect("valid rego");
        engine
    }

    // ── Pipeline execution tests ──

    #[tokio::test]
    async fn pipeline_executes_allowed_action() {
        let pipeline = mock_pipeline();
        let mut ctx = test_ctx();
        let mut action_ctx = test_action_ctx();
        let engine = test_policy_engine();
        let mut prompt = PromptEnvelope::build("system", "user input");

        let result = pipeline
            .execute(&mut ctx, &mut action_ctx, &engine, &mut prompt)
            .await;
        assert!(result.is_ok());
        assert!(ctx.skill_eval.is_some());
        assert!(ctx.model_result.is_some());
    }

    #[tokio::test]
    async fn pipeline_rejects_denied_action() {
        let pipeline = mock_pipeline();
        let mut ctx = test_ctx();
        let mut action_ctx = test_action_ctx();
        let engine = deny_policy_engine();
        let mut prompt = PromptEnvelope::build("system", "user input");

        let result = pipeline
            .execute(&mut ctx, &mut action_ctx, &engine, &mut prompt)
            .await;
        assert!(matches!(result, Err(AgentError::PolicyDenied(_))));
        // LLM should NOT have been called
        assert!(ctx.model_result.is_none());
    }

    #[tokio::test]
    async fn pipeline_runs_middleware_in_order() {
        use std::sync::atomic::{AtomicU32, Ordering};

        #[derive(Debug)]
        struct OrderTracker {
            id: u32,
            counter: Arc<AtomicU32>,
            before_llm_order: std::sync::Mutex<Option<u32>>,
        }

        #[async_trait]
        impl AgentMiddleware for OrderTracker {
            async fn before_llm(
                &self,
                _ctx: &mut PipelineContext,
                _prompt: &mut PromptEnvelope,
            ) -> Result<(), AgentError> {
                let order = self.counter.fetch_add(1, Ordering::SeqCst);
                *self.before_llm_order.lock().unwrap() = Some(order);
                Ok(())
            }
        }

        let counter = Arc::new(AtomicU32::new(0));
        let mw1 = Arc::new(OrderTracker {
            id: 1,
            counter: counter.clone(),
            before_llm_order: std::sync::Mutex::new(None),
        });
        let mw2 = Arc::new(OrderTracker {
            id: 2,
            counter: counter.clone(),
            before_llm_order: std::sync::Mutex::new(None),
        });

        let pipeline = mock_pipeline()
            .with_middleware(mw1.clone() as Arc<dyn AgentMiddleware>)
            .with_middleware(mw2.clone() as Arc<dyn AgentMiddleware>);

        let mut ctx = test_ctx();
        let mut action_ctx = test_action_ctx();
        let engine = test_policy_engine();
        let mut prompt = PromptEnvelope::build("system", "user input");

        pipeline
            .execute(&mut ctx, &mut action_ctx, &engine, &mut prompt)
            .await
            .unwrap();

        assert_eq!(*mw1.before_llm_order.lock().unwrap(), Some(0));
        assert_eq!(*mw2.before_llm_order.lock().unwrap(), Some(1));
    }

    // ── Token budget tests ──

    #[test]
    fn token_budget_starts_at_zero() {
        let mw = TokenBudgetMiddleware::new(50_000);
        assert_eq!(mw.usage_for("enc-001"), 0);
    }

    #[tokio::test]
    async fn token_budget_rejects_over_limit() {
        let mw = TokenBudgetMiddleware::new(100);
        // Simulate prior usage
        mw.usage
            .lock()
            .unwrap()
            .insert("enc-001".to_string(), 101);

        let mut ctx = test_ctx();
        let mut prompt = PromptEnvelope::build("system", "user input");
        let result = mw.before_llm(&mut ctx, &mut prompt).await;
        assert!(matches!(result, Err(AgentError::ClaudeApi(_))));
    }

    #[tokio::test]
    async fn token_budget_allows_under_limit() {
        let mw = TokenBudgetMiddleware::new(50_000);
        let mut ctx = test_ctx();
        let mut prompt = PromptEnvelope::build("system", "user input");
        let result = mw.before_llm(&mut ctx, &mut prompt).await;
        assert!(result.is_ok());
    }

    // ── PHI audit tests ──

    #[test]
    fn phi_audit_detects_ssn_pattern() {
        let mw = PhiAuditMiddleware::new();
        let findings = mw.check_for_phi("Patient SSN is 123-45-6789 and needs care");
        assert!(!findings.is_empty());
        assert!(findings[0].contains("SSN"));
    }

    #[test]
    fn phi_audit_detects_dob_indicator() {
        let mw = PhiAuditMiddleware::new();
        let findings = mw.check_for_phi("DOB: 1990-01-15");
        assert!(!findings.is_empty());
    }

    #[test]
    fn phi_audit_detects_mrn_indicator() {
        let mw = PhiAuditMiddleware::new();
        let findings = mw.check_for_phi("MRN: 12345678");
        assert!(!findings.is_empty());
    }

    #[test]
    fn phi_audit_clean_prompt_passes() {
        let mw = PhiAuditMiddleware::new();
        let findings = mw.check_for_phi("Patient presents with chest pain, HR 110, BP 90/60");
        assert!(findings.is_empty());
    }

    #[tokio::test]
    async fn phi_audit_middleware_does_not_block() {
        // PHI audit is warn-only, should not return error
        let mw = PhiAuditMiddleware::new();
        let mut ctx = test_ctx();
        let mut prompt =
            PromptEnvelope::build("system", "Patient DOB: 1990-01-15 with chest pain");
        let result = mw.before_llm(&mut ctx, &mut prompt).await;
        assert!(result.is_ok()); // warn-only, does not block
    }
}
