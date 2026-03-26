use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// What kind of input/output the model handles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelModality {
    TextToText,
    TextToStructured,
    ImageToText,
    TabularToScore,
}

/// Approval lifecycle for a registered model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Approved,
    Pending,
    Revoked { reason: String },
    Experimental,
}

/// Provenance metadata for a registered model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProvenance {
    pub vendor: String,
    pub training_data_hash: Option<String>,
    pub approval_status: ApprovalStatus,
    pub approved_by: Option<String>,
    pub approved_at: Option<String>,
    pub regulatory_class: Option<String>,
}

/// Metadata wrapping every LLM response — confidence, latency, token usage.
///
/// Aligned with VERITAS ModelCapability RFC: every invocation surfaces
/// structured observability metadata alongside the text output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResult {
    pub output: String,
    pub confidence: Option<f64>,
    pub latency_ms: u64,
    pub token_usage: Option<TokenUsage>,
    pub model_id: String,
    pub model_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Identity card for a registered model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredModel {
    pub model_id: String,
    pub modality: ModelModality,
    pub version: String,
    pub provenance: ModelProvenance,
}

/// Drift status returned by the drift monitor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DriftStatus {
    Stable,
    Warning {
        metric: String,
        current: f64,
        threshold: f64,
    },
    Drifted {
        metric: String,
        current: f64,
        threshold: f64,
    },
}

/// Configuration for drift detection.
#[derive(Debug, Clone)]
pub struct DriftConfig {
    pub window_size: usize,
    pub warning_threshold: f64,
    pub drift_threshold: f64,
}

impl Default for DriftConfig {
    fn default() -> Self {
        Self {
            window_size: 100,
            warning_threshold: 0.1,
            drift_threshold: 0.2,
        }
    }
}

impl DriftConfig {
    pub fn new(
        window_size: usize,
        warning_threshold: f64,
        drift_threshold: f64,
    ) -> Result<Self, crate::error::AgentError> {
        let cfg = Self {
            window_size,
            warning_threshold,
            drift_threshold,
        };
        cfg.validate()?;
        Ok(cfg)
    }

    /// Validate configuration values. Called by `new()` and can be called
    /// independently after constructing via struct literal.
    pub fn validate(&self) -> Result<(), crate::error::AgentError> {
        if self.window_size == 0 {
            return Err(crate::error::AgentError::ModelRegistry(
                "window_size must be > 0".into(),
            ));
        }
        if !(0.0..=1.0).contains(&self.warning_threshold) {
            return Err(crate::error::AgentError::ModelRegistry(
                "warning_threshold must be in [0.0, 1.0]".into(),
            ));
        }
        if !(0.0..=1.0).contains(&self.drift_threshold) {
            return Err(crate::error::AgentError::ModelRegistry(
                "drift_threshold must be in [0.0, 1.0]".into(),
            ));
        }
        if self.warning_threshold > self.drift_threshold {
            return Err(crate::error::AgentError::ModelRegistry(
                "warning_threshold must be <= drift_threshold".into(),
            ));
        }
        Ok(())
    }
}

/// In-memory drift monitor — tracks rolling confidence scores per model.
///
/// Uses interior mutability (Mutex) so it can be shared via Arc across
/// async tasks without requiring `&mut self` on hot paths.
pub struct InMemoryDriftMonitor {
    config: DriftConfig,
    // model_id -> (baseline_mean, recent_scores)
    state: Mutex<HashMap<String, DriftState>>,
}

struct DriftState {
    baseline_mean: Option<f64>,
    scores: Vec<f64>,
}

impl std::fmt::Debug for InMemoryDriftMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemoryDriftMonitor")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl InMemoryDriftMonitor {
    pub fn new(config: DriftConfig) -> Self {
        Self {
            config,
            state: Mutex::new(HashMap::new()),
        }
    }

    /// Construct with default drift configuration (window=100, warn=10%, drift=20%).
    pub fn with_defaults() -> Self {
        Self::new(DriftConfig::default())
    }

    /// Record a confidence score for a model invocation.
    pub fn record(&self, model_id: &str, confidence: f64) -> Result<(), crate::error::AgentError> {
        if !(0.0..=1.0).contains(&confidence) {
            return Err(crate::error::AgentError::ModelRegistry(format!(
                "confidence must be in [0.0, 1.0], got {confidence}"
            )));
        }
        let mut state = self.state.lock().map_err(|e| {
            crate::error::AgentError::ModelRegistry(format!(
                "drift monitor lock poisoned: {e}"
            ))
        })?;
        let entry = state
            .entry(model_id.to_string())
            .or_insert_with(|| DriftState {
                baseline_mean: None,
                scores: Vec::new(),
            });

        entry.scores.push(confidence);

        // Set baseline from the first window_size observations.
        if entry.baseline_mean.is_none() && entry.scores.len() >= self.config.window_size {
            let sum: f64 = entry.scores.iter().sum();
            entry.baseline_mean = Some(sum / entry.scores.len() as f64);
        }

        // Keep only the most recent window_size * 2 scores to bound memory.
        if entry.scores.len() > self.config.window_size * 2 {
            let drain_to = entry.scores.len() - self.config.window_size;
            entry.scores.drain(..drain_to);
        }

        Ok(())
    }

    /// Check drift status for a model.
    ///
    /// Returns `Stable` if not enough data has been collected yet.
    /// Compares the mean of the most recent window against the established
    /// baseline; reports `Warning` or `Drifted` on confidence degradation.
    pub fn check_drift(&self, model_id: &str) -> DriftStatus {
        let state = match self.state.lock() {
            Ok(s) => s,
            // Poisoned lock — fail safe rather than panic.
            Err(_) => return DriftStatus::Stable,
        };

        let entry = match state.get(model_id) {
            Some(e) => e,
            None => return DriftStatus::Stable,
        };

        let baseline = match entry.baseline_mean {
            Some(b) => b,
            // Not enough data yet.
            None => return DriftStatus::Stable,
        };

        if entry.scores.is_empty() {
            return DriftStatus::Stable;
        }

        // Compute mean of the most recent window.
        let recent_start = entry.scores.len().saturating_sub(self.config.window_size);
        let recent = &entry.scores[recent_start..];
        let recent_mean: f64 = recent.iter().sum::<f64>() / recent.len() as f64;

        let degradation = baseline - recent_mean;

        if degradation >= self.config.drift_threshold {
            DriftStatus::Drifted {
                metric: "confidence_degradation".into(),
                current: degradation,
                threshold: self.config.drift_threshold,
            }
        } else if degradation >= self.config.warning_threshold {
            DriftStatus::Warning {
                metric: "confidence_degradation".into(),
                current: degradation,
                threshold: self.config.warning_threshold,
            }
        } else {
            DriftStatus::Stable
        }
    }
}

/// Central model registry — deny-by-default.
///
/// Unknown or revoked models cannot be invoked. Integrates with the VERITAS
/// policy engine via `capabilities_for()`, which maps approval state to
/// capability strings the OPA Rego policies can gate on.
pub struct ModelRegistry {
    models: HashMap<String, RegisteredModel>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    /// Register a model. Returns an error on duplicate `model_id`.
    pub fn register(&mut self, model: RegisteredModel) -> Result<(), crate::error::AgentError> {
        if self.models.contains_key(&model.model_id) {
            return Err(crate::error::AgentError::ModelRegistry(format!(
                "model '{}' already registered",
                model.model_id
            )));
        }
        self.models.insert(model.model_id.clone(), model);
        Ok(())
    }

    /// Returns `true` only if the model is registered AND has `Approved` status.
    pub fn is_approved(&self, model_id: &str) -> bool {
        self.models
            .get(model_id)
            .map(|m| matches!(m.provenance.approval_status, ApprovalStatus::Approved))
            .unwrap_or(false)
    }

    /// Look up a registered model by ID.
    pub fn get(&self, model_id: &str) -> Option<&RegisteredModel> {
        self.models.get(model_id)
    }

    /// Revoke a model's approval. Does not remove it from the registry so the
    /// audit record remains intact.
    pub fn revoke(
        &mut self,
        model_id: &str,
        reason: &str,
    ) -> Result<(), crate::error::AgentError> {
        let model = self.models.get_mut(model_id).ok_or_else(|| {
            crate::error::AgentError::ModelRegistry(format!(
                "model '{model_id}' not registered"
            ))
        })?;
        model.provenance.approval_status = ApprovalStatus::Revoked {
            reason: reason.to_string(),
        };
        Ok(())
    }

    /// Filter models by modality.
    pub fn by_modality(&self, modality: &ModelModality) -> Vec<&RegisteredModel> {
        self.models
            .values()
            .filter(|m| &m.modality == modality)
            .collect()
    }

    /// Check drift via the monitor and auto-revoke the model if it has drifted.
    ///
    /// This is the primary feedback loop between the drift monitor and the
    /// registry. Called after batches of invocations to enforce governance.
    pub fn check_and_update(
        &mut self,
        model_id: &str,
        monitor: &InMemoryDriftMonitor,
    ) -> Result<DriftStatus, crate::error::AgentError> {
        let status = monitor.check_drift(model_id);
        if let DriftStatus::Drifted {
            ref metric,
            current,
            ..
        } = status
        {
            tracing::warn!(
                model_id,
                metric = metric.as_str(),
                degradation = current,
                "model drifted — auto-revoking"
            );
            self.revoke(
                model_id,
                &format!("auto-revoked: {metric} degradation {current:.4}"),
            )?;
        }
        Ok(status)
    }

    /// Generate capability strings for policy engine integration.
    ///
    /// The returned strings are passed to the OPA Rego policy engine so that
    /// policies can gate actions on model approval state without knowing
    /// internal registry structure.
    pub fn capabilities_for(&self, model_id: &str) -> Vec<String> {
        let mut caps = vec![format!("model:{model_id}")];
        if self.is_approved(model_id) {
            caps.push("model:approved".to_string());
        }
        caps
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_model(id: &str) -> RegisteredModel {
        RegisteredModel {
            model_id: id.to_string(),
            modality: ModelModality::TextToText,
            version: "1.0.0".to_string(),
            provenance: ModelProvenance {
                vendor: "Anthropic".to_string(),
                training_data_hash: None,
                approval_status: ApprovalStatus::Approved,
                approved_by: Some("admin".to_string()),
                approved_at: Some("2026-03-23".to_string()),
                regulatory_class: None,
            },
        }
    }

    #[test]
    fn registry_register_and_lookup() {
        let mut reg = ModelRegistry::new();
        reg.register(test_model("claude-sonnet-4-6")).unwrap();
        assert!(reg.get("claude-sonnet-4-6").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn registry_duplicate_errors() {
        let mut reg = ModelRegistry::new();
        reg.register(test_model("claude-sonnet-4-6")).unwrap();
        assert!(reg.register(test_model("claude-sonnet-4-6")).is_err());
    }

    #[test]
    fn registry_is_approved() {
        let mut reg = ModelRegistry::new();
        reg.register(test_model("claude-sonnet-4-6")).unwrap();
        assert!(reg.is_approved("claude-sonnet-4-6"));
        assert!(!reg.is_approved("unknown"));
    }

    #[test]
    fn registry_revoke() {
        let mut reg = ModelRegistry::new();
        reg.register(test_model("claude-sonnet-4-6")).unwrap();
        reg.revoke("claude-sonnet-4-6", "testing").unwrap();
        assert!(!reg.is_approved("claude-sonnet-4-6"));
    }

    #[test]
    fn registry_capabilities_for() {
        let mut reg = ModelRegistry::new();
        reg.register(test_model("claude-sonnet-4-6")).unwrap();
        let caps = reg.capabilities_for("claude-sonnet-4-6");
        assert!(caps.contains(&"model:claude-sonnet-4-6".to_string()));
        assert!(caps.contains(&"model:approved".to_string()));
    }

    #[test]
    fn registry_capabilities_for_revoked() {
        let mut reg = ModelRegistry::new();
        reg.register(test_model("claude-sonnet-4-6")).unwrap();
        reg.revoke("claude-sonnet-4-6", "test").unwrap();
        let caps = reg.capabilities_for("claude-sonnet-4-6");
        assert!(caps.contains(&"model:claude-sonnet-4-6".to_string()));
        assert!(!caps.contains(&"model:approved".to_string()));
    }

    #[test]
    fn registry_by_modality() {
        let mut reg = ModelRegistry::new();
        reg.register(test_model("m1")).unwrap();
        let mut m2 = test_model("m2");
        m2.modality = ModelModality::ImageToText;
        reg.register(m2).unwrap();
        assert_eq!(reg.by_modality(&ModelModality::TextToText).len(), 1);
        assert_eq!(reg.by_modality(&ModelModality::ImageToText).len(), 1);
    }

    #[test]
    fn drift_config_validates() {
        assert!(DriftConfig::new(10, 0.1, 0.3).is_ok());
        assert!(DriftConfig::new(0, 0.1, 0.3).is_err());
        assert!(DriftConfig::new(10, -0.1, 0.3).is_err());
        assert!(DriftConfig::new(10, 0.1, 1.5).is_err());
    }

    #[test]
    fn drift_monitor_stable_initially() {
        let config = DriftConfig::new(5, 0.1, 0.3).unwrap();
        let monitor = InMemoryDriftMonitor::new(config);
        assert_eq!(monitor.check_drift("m1"), DriftStatus::Stable);
    }

    #[test]
    fn drift_monitor_detects_degradation() {
        let config = DriftConfig::new(5, 0.05, 0.15).unwrap();
        let monitor = InMemoryDriftMonitor::new(config);
        // Build baseline with high confidence.
        for _ in 0..5 {
            monitor.record("m1", 0.95).unwrap();
        }
        assert_eq!(monitor.check_drift("m1"), DriftStatus::Stable);

        // Now degrade confidence significantly.
        for _ in 0..5 {
            monitor.record("m1", 0.75).unwrap();
        }
        assert!(
            matches!(monitor.check_drift("m1"), DriftStatus::Drifted { .. }),
            "expected Drifted, got {:?}",
            monitor.check_drift("m1")
        );
    }

    #[test]
    fn drift_auto_revoke() {
        let config = DriftConfig::new(3, 0.05, 0.15).unwrap();
        let monitor = InMemoryDriftMonitor::new(config);
        let mut reg = ModelRegistry::new();
        reg.register(test_model("m1")).unwrap();

        // Build baseline.
        for _ in 0..3 {
            monitor.record("m1", 0.90).unwrap();
        }
        // Degrade below drift threshold.
        for _ in 0..3 {
            monitor.record("m1", 0.60).unwrap();
        }

        let status = reg.check_and_update("m1", &monitor).unwrap();
        assert!(matches!(status, DriftStatus::Drifted { .. }));
        assert!(!reg.is_approved("m1"));
    }
}
