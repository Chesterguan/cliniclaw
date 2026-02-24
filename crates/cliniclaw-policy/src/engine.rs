use tracing::{info, warn};

use crate::{
    context::ActionContext,
    decision::PolicyDecision,
    error::PolicyError,
    rule::{PolicyFile, PolicyRule},
    skill::{ClinicalSkillSpec, Severity, SkillPolicyFile},
};

/// The result of a skill-aware policy evaluation.
#[derive(Debug, Clone)]
pub struct SkillEvaluation {
    pub decision: PolicyDecision,
    /// SHA-256 hash of the canonical skill definition (for audit trail)
    pub spec_hash: Option<String>,
    /// The skill ID that matched
    pub skill_id: Option<String>,
    /// The severity level of the matched skill
    pub severity: Option<Severity>,
}

#[derive(Debug, Clone, Default)]
pub struct PolicyEngine {
    rules: Vec<PolicyRule>,
    skills: Vec<ClinicalSkillSpec>,
}

impl PolicyEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load policy rules from a TOML string (backward-compatible, [[rule]] only).
    pub fn load_from_str(&mut self, toml_str: &str) -> Result<(), PolicyError> {
        let policy_file: PolicyFile = toml::from_str(toml_str)
            .map_err(|e| PolicyError::LoadError(e.to_string()))?;

        for rule in &policy_file.rules {
            if rule.name.is_empty() {
                return Err(PolicyError::InvalidRule(
                    "rule name must not be empty".to_string(),
                ));
            }
            if rule.action.is_empty() {
                return Err(PolicyError::InvalidRule(format!(
                    "rule '{}' has an empty action pattern",
                    rule.name
                )));
            }
        }

        let count = policy_file.rules.len();
        self.rules.extend(policy_file.rules);
        info!(loaded_rules = count, total_rules = self.rules.len(), "policy rules loaded");
        Ok(())
    }

    pub fn load_from_file(&mut self, path: impl AsRef<std::path::Path>) -> Result<(), PolicyError> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .map_err(|e| PolicyError::LoadError(format!("{}: {}", path.display(), e)))?;
        self.load_from_str(&contents)
    }

    /// Load both [[rule]] and [[skill]] sections from a TOML file.
    pub fn load_skills_from_file(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), PolicyError> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .map_err(|e| PolicyError::LoadError(format!("{}: {}", path.display(), e)))?;
        self.load_skills_from_str(&contents)
    }

    /// Load both [[rule]] and [[skill]] sections from a unified TOML string.
    pub fn load_skills_from_str(&mut self, toml_str: &str) -> Result<(), PolicyError> {
        let file: SkillPolicyFile =
            toml::from_str(toml_str).map_err(|e| PolicyError::LoadError(e.to_string()))?;

        for rule in &file.rules {
            if rule.name.is_empty() {
                return Err(PolicyError::InvalidRule(
                    "rule name must not be empty".to_string(),
                ));
            }
            if rule.action.is_empty() {
                return Err(PolicyError::InvalidRule(format!(
                    "rule '{}' has an empty action pattern",
                    rule.name
                )));
            }
        }

        for skill in &file.skills {
            skill.validate()?;
        }

        let rule_count = file.rules.len();
        let skill_count = file.skills.len();
        self.rules.extend(file.rules);

        let mut skills = file.skills;
        for skill in &mut skills {
            skill.spec_hash = skill.compute_spec_hash();
        }
        self.skills.extend(skills);

        info!(
            loaded_rules = rule_count,
            loaded_skills = skill_count,
            total_rules = self.rules.len(),
            total_skills = self.skills.len(),
            "policy rules and skills loaded"
        );
        Ok(())
    }

    /// Evaluate an action context against all loaded rules.
    /// Deny-by-default: if no rule matches, action is denied.
    /// This method is unchanged from the original for backward compatibility.
    pub fn evaluate(&self, context: &ActionContext) -> PolicyDecision {
        let mut matched: Vec<&PolicyRule> = self
            .rules
            .iter()
            .filter(|rule| Self::action_matches(&rule.action, &context.action))
            .collect();

        if matched.is_empty() {
            warn!(
                action = %context.action,
                actor_id = %context.actor_id,
                "policy deny-by-default: no rule matched action"
            );
            return PolicyDecision::Deny;
        }

        // Highest priority first
        matched.sort_by(|a, b| b.priority.cmp(&a.priority));

        // Iterate through rules in priority order. A rule only applies if ALL
        // its conditions and capability requirements are met. If a rule's
        // conditions don't match, skip to the next rule (lower priority).
        'rules: for rule in &matched {
            // Check required capabilities
            let mut caps_ok = true;
            for cap in &rule.required_capabilities {
                if !context.capabilities.iter().any(|c| c == cap) {
                    caps_ok = false;
                    break;
                }
            }
            if !caps_ok {
                continue 'rules;
            }

            // Check conditions
            let mut conditions_ok = true;
            for (key, expected) in &rule.conditions {
                match context.properties.get(key) {
                    Some(actual) if actual == expected => {}
                    _ => {
                        conditions_ok = false;
                        break;
                    }
                }
            }
            if !conditions_ok {
                continue 'rules;
            }

            // All conditions and capabilities match — apply this rule
            info!(
                action = %context.action,
                actor_id = %context.actor_id,
                rule = %rule.name,
                decision = ?rule.decision,
                "policy decision"
            );
            return rule.decision.clone();
        }

        // No rule fully matched — deny by default
        warn!(
            action = %context.action,
            actor_id = %context.actor_id,
            "policy deny: no rule conditions fully matched"
        );
        PolicyDecision::Deny
    }

    /// Skill-aware evaluation. Checks skill metadata (role, capability tokens,
    /// population) before falling through to policy rule evaluation.
    ///
    /// Order: skill lookup → role check → capability token validation →
    /// population gate → policy rule evaluation → severity escalation.
    pub fn evaluate_with_skill(
        &self,
        context: &ActionContext,
    ) -> Result<SkillEvaluation, PolicyError> {
        let skill = self.find_skill(&context.action);

        match skill {
            None => {
                // No skill defined — fall back to basic evaluation
                let decision = self.evaluate(context);
                Ok(SkillEvaluation {
                    decision,
                    spec_hash: None,
                    skill_id: None,
                    severity: None,
                })
            }
            Some(skill) => {
                // Step 1: Role check
                if let Some(ref role) = context.role {
                    if !skill.is_role_allowed(role) {
                        warn!(
                            action = %context.action,
                            actor_id = %context.actor_id,
                            role = %role,
                            skill_id = %skill.id,
                            "role not allowed for skill"
                        );
                        return Err(PolicyError::RoleNotAllowed {
                            role: role.clone(),
                            skill_id: skill.id.clone(),
                        });
                    }
                }

                // Step 2: Capability token validation
                for required_cap in &skill.required_capabilities {
                    let has_bare = context.capabilities.iter().any(|c| c == required_cap);
                    let matching_token = context
                        .capability_tokens
                        .iter()
                        .find(|t| t.name == *required_cap);

                    if !has_bare && matching_token.is_none() {
                        return Err(PolicyError::MissingCapability {
                            capability: required_cap.clone(),
                            actor_id: context.actor_id.clone(),
                        });
                    }

                    // If structured token exists, validate expiry/actor/scope
                    if let Some(token) = matching_token {
                        token.validate_for_context(
                            &context.actor_id,
                            context.patient_id.as_deref(),
                            context.encounter_id.as_deref(),
                        )?;
                    }
                }

                // Step 3: Population gate
                skill.check_population(&context.properties)?;

                // Step 4: Policy rule evaluation
                let mut decision = self.evaluate(context);

                // Step 5: Severity escalation — critical always requires approval
                if skill.severity == Severity::Critical && decision == PolicyDecision::Allow {
                    info!(
                        action = %context.action,
                        skill_id = %skill.id,
                        "critical severity: escalating Allow to RequireApproval"
                    );
                    decision = PolicyDecision::RequireApproval;
                }

                Ok(SkillEvaluation {
                    decision,
                    spec_hash: Some(skill.spec_hash.clone()),
                    skill_id: Some(skill.id.clone()),
                    severity: Some(skill.severity.clone()),
                })
            }
        }
    }

    /// Look up a skill spec by action (exact match or wildcard prefix).
    pub fn find_skill(&self, action: &str) -> Option<&ClinicalSkillSpec> {
        self.skills.iter().find(|s| {
            s.action == action
                || s.action
                    .strip_suffix(".*")
                    .map_or(false, |prefix| action.starts_with(&format!("{prefix}.")))
        })
    }

    /// Get all loaded skill specs.
    pub fn skills(&self) -> &[ClinicalSkillSpec] {
        &self.skills
    }

    fn action_matches(pattern: &str, action: &str) -> bool {
        if let Some(prefix) = pattern.strip_suffix(".*") {
            action.starts_with(&format!("{prefix}."))
        } else {
            pattern == action
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ActionContext;
    use crate::decision::PolicyDecision;

    fn engine_with(toml: &str) -> PolicyEngine {
        let mut engine = PolicyEngine::new();
        engine.load_from_str(toml).expect("valid TOML");
        engine
    }

    fn ctx(action: &str, actor_id: &str) -> ActionContext {
        ActionContext::new(action, actor_id)
    }

    fn ctx_with_caps(action: &str, actor_id: &str, caps: &[&str]) -> ActionContext {
        let mut c = ctx(action, actor_id);
        c.capabilities = caps.iter().map(|s| s.to_string()).collect();
        c
    }

    // ── Existing tests (unchanged) ──────────────────────────────────

    #[test]
    fn deny_by_default_no_rules() {
        let engine = PolicyEngine::new();
        let context = ctx("ambient_doc.generate_note", "practitioner-1");
        assert_eq!(engine.evaluate(&context), PolicyDecision::Deny);
    }

    #[test]
    fn allow_with_matching_capability() {
        let engine = engine_with(
            r#"
            [[rule]]
            name = "allow_note_gen"
            action = "ambient_doc.generate_note"
            decision = "allow"
            required_capabilities = ["note_generation"]
            priority = 10
            "#,
        );
        let mut context = ctx("ambient_doc.generate_note", "practitioner-1");
        context.capabilities = vec!["note_generation".to_string()];
        assert_eq!(engine.evaluate(&context), PolicyDecision::Allow);
    }

    #[test]
    fn deny_missing_required_capability() {
        let engine = engine_with(
            r#"
            [[rule]]
            name = "allow_note_gen"
            action = "ambient_doc.generate_note"
            decision = "allow"
            required_capabilities = ["note_generation"]
            priority = 10
            "#,
        );
        let context = ctx("ambient_doc.generate_note", "practitioner-1");
        assert_eq!(engine.evaluate(&context), PolicyDecision::Deny);
    }

    #[test]
    fn wildcard_action_matches_namespace() {
        let engine = engine_with(
            r#"
            [[rule]]
            name = "allow_all_order_entry"
            action = "order_entry.*"
            decision = "allow"
            required_capabilities = ["order_entry"]
            priority = 10
            "#,
        );
        let context = ctx_with_caps("order_entry.propose", "practitioner-1", &["order_entry"]);
        assert_eq!(engine.evaluate(&context), PolicyDecision::Allow);

        let context2 = ctx_with_caps("order_entry.review", "practitioner-1", &["order_entry"]);
        assert_eq!(engine.evaluate(&context2), PolicyDecision::Allow);
    }

    #[test]
    fn wildcard_does_not_match_different_namespace() {
        let engine = engine_with(
            r#"
            [[rule]]
            name = "allow_all_order_entry"
            action = "order_entry.*"
            decision = "allow"
            required_capabilities = ["order_entry"]
            priority = 10
            "#,
        );
        let context = ctx_with_caps("ambient_doc.generate_note", "practitioner-1", &["order_entry"]);
        assert_eq!(engine.evaluate(&context), PolicyDecision::Deny);
    }

    #[test]
    fn higher_priority_rule_wins() {
        let engine = engine_with(
            r#"
            [[rule]]
            name = "low_priority_allow"
            action = "ambient_doc.generate_note"
            decision = "allow"
            required_capabilities = ["note_generation"]
            priority = 10

            [[rule]]
            name = "high_priority_deny"
            action = "ambient_doc.generate_note"
            decision = "deny"
            priority = 20
            "#,
        );
        let context =
            ctx_with_caps("ambient_doc.generate_note", "practitioner-1", &["note_generation"]);
        assert_eq!(engine.evaluate(&context), PolicyDecision::Deny);
    }

    #[test]
    fn require_approval_decision() {
        let engine = engine_with(
            r#"
            [[rule]]
            name = "require_approval_high_risk"
            action = "order_entry.propose_high_risk"
            decision = "require_approval"
            required_capabilities = ["order_entry"]
            priority = 20
            "#,
        );
        let context =
            ctx_with_caps("order_entry.propose_high_risk", "practitioner-1", &["order_entry"]);
        assert_eq!(engine.evaluate(&context), PolicyDecision::RequireApproval);
    }

    #[test]
    fn condition_match_allows_action() {
        let engine = engine_with(
            r#"
            [[rule]]
            name = "allow_note_gen_in_progress"
            action = "ambient_doc.generate_note"
            decision = "allow"
            required_capabilities = ["note_generation"]
            priority = 10

            [rule.conditions]
            encounter_status = "in-progress"
            "#,
        );
        let mut context =
            ctx_with_caps("ambient_doc.generate_note", "practitioner-1", &["note_generation"]);
        context
            .properties
            .insert("encounter_status".to_string(), "in-progress".to_string());
        assert_eq!(engine.evaluate(&context), PolicyDecision::Allow);
    }

    #[test]
    fn condition_mismatch_denies_action() {
        let engine = engine_with(
            r#"
            [[rule]]
            name = "allow_note_gen_in_progress"
            action = "ambient_doc.generate_note"
            decision = "allow"
            required_capabilities = ["note_generation"]
            priority = 10

            [rule.conditions]
            encounter_status = "in-progress"
            "#,
        );
        let mut context =
            ctx_with_caps("ambient_doc.generate_note", "practitioner-1", &["note_generation"]);
        context
            .properties
            .insert("encounter_status".to_string(), "finished".to_string());
        assert_eq!(engine.evaluate(&context), PolicyDecision::Deny);
    }

    #[test]
    fn condition_key_absent_denies_action() {
        let engine = engine_with(
            r#"
            [[rule]]
            name = "allow_note_gen_in_progress"
            action = "ambient_doc.generate_note"
            decision = "allow"
            required_capabilities = ["note_generation"]
            priority = 10

            [rule.conditions]
            encounter_status = "in-progress"
            "#,
        );
        let context =
            ctx_with_caps("ambient_doc.generate_note", "practitioner-1", &["note_generation"]);
        assert_eq!(engine.evaluate(&context), PolicyDecision::Deny);
    }

    #[test]
    fn invalid_toml_returns_load_error() {
        let mut engine = PolicyEngine::new();
        let result = engine.load_from_str("this is not valid toml !!!");
        assert!(matches!(result, Err(PolicyError::LoadError(_))));
    }

    // ── New skill-aware evaluation tests ────────────────────────────

    fn skill_engine(toml: &str) -> PolicyEngine {
        let mut engine = PolicyEngine::new();
        engine.load_skills_from_str(toml).expect("valid TOML");
        engine
    }

    const SKILL_TOML: &str = r#"
        [[rule]]
        name = "allow_note_gen"
        action = "ambient_doc.generate_note"
        decision = "allow"
        required_capabilities = ["note_generation"]
        priority = 10

        [[skill]]
        id = "ambient_doc.generate_note"
        name = "Ambient Doc"
        version = "1.0.0"
        severity = "medium"
        allowed_roles = ["physician", "nurse_practitioner"]
        action = "ambient_doc.generate_note"
        required_capabilities = ["note_generation"]

        [skill.audit]
        intent = "Generate clinical notes from transcripts"
        rationale = "Documentation efficiency"

        [skill.audit.provenance]
        type = "sop"
        reference = "ClinicClaw SOP v1"

        [skill.population]
        include = ["patient.active == true"]
        exclude = ["patient.deceased == true"]
    "#;

    #[test]
    fn evaluate_with_skill_role_denied() {
        let engine = skill_engine(SKILL_TOML);
        let mut c = ctx("ambient_doc.generate_note", "actor-1");
        c.capabilities = vec!["note_generation".into()];
        c.role = Some("receptionist".into());

        let result = engine.evaluate_with_skill(&c);
        assert!(matches!(result, Err(PolicyError::RoleNotAllowed { .. })));
    }

    #[test]
    fn evaluate_with_skill_role_allowed() {
        let engine = skill_engine(SKILL_TOML);
        let mut c = ctx("ambient_doc.generate_note", "actor-1");
        c.capabilities = vec!["note_generation".into()];
        c.role = Some("physician".into());
        c.properties.insert("patient.active".into(), "true".into());
        c.properties
            .insert("patient.deceased".into(), "false".into());

        let result = engine.evaluate_with_skill(&c).unwrap();
        assert_eq!(result.decision, PolicyDecision::Allow);
    }

    #[test]
    fn evaluate_with_skill_population_excluded() {
        let engine = skill_engine(SKILL_TOML);
        let mut c = ctx("ambient_doc.generate_note", "actor-1");
        c.capabilities = vec!["note_generation".into()];
        c.properties.insert("patient.active".into(), "true".into());
        c.properties
            .insert("patient.deceased".into(), "true".into());

        let result = engine.evaluate_with_skill(&c);
        assert!(matches!(
            result,
            Err(PolicyError::PopulationExcluded { .. })
        ));
    }

    #[test]
    fn evaluate_with_skill_missing_capability() {
        let engine = skill_engine(SKILL_TOML);
        let c = ctx("ambient_doc.generate_note", "actor-1");
        // No capabilities at all

        let result = engine.evaluate_with_skill(&c);
        assert!(matches!(
            result,
            Err(PolicyError::MissingCapability { .. })
        ));
    }

    #[test]
    fn evaluate_with_skill_critical_escalates() {
        let engine = skill_engine(
            r#"
            [[rule]]
            name = "allow_controlled"
            action = "order_entry.propose_controlled"
            decision = "allow"
            required_capabilities = ["order_entry"]
            priority = 10

            [[skill]]
            id = "order_entry.propose_controlled"
            name = "Controlled Substance Order"
            version = "1.0.0"
            severity = "critical"
            action = "order_entry.propose_controlled"
            required_capabilities = ["order_entry"]

            [skill.audit]
            intent = "Order controlled substances with mandatory oversight"
            rationale = "DEA Schedule II requires dual sign-off"

            [skill.audit.provenance]
            type = "regulatory"
            reference = "DEA 21 CFR Part 1306"
        "#,
        );

        let mut c = ctx("order_entry.propose_controlled", "actor-1");
        c.capabilities = vec!["order_entry".into()];

        let result = engine.evaluate_with_skill(&c).unwrap();
        assert_eq!(result.decision, PolicyDecision::RequireApproval);
        assert_eq!(
            result.skill_id,
            Some("order_entry.propose_controlled".into())
        );
        assert_eq!(result.severity, Some(Severity::Critical));
    }

    #[test]
    fn evaluate_with_skill_fallback_when_no_skill() {
        // Only rules loaded, no skills
        let engine = engine_with(
            r#"
            [[rule]]
            name = "allow_note_gen"
            action = "ambient_doc.generate_note"
            decision = "allow"
            required_capabilities = ["note_generation"]
            priority = 10
            "#,
        );

        let mut c = ctx("ambient_doc.generate_note", "actor-1");
        c.capabilities = vec!["note_generation".into()];

        let result = engine.evaluate_with_skill(&c).unwrap();
        assert_eq!(result.decision, PolicyDecision::Allow);
        assert!(result.spec_hash.is_none());
        assert!(result.skill_id.is_none());
    }

    #[test]
    fn evaluate_with_skill_returns_spec_hash() {
        let engine = skill_engine(SKILL_TOML);
        let mut c = ctx("ambient_doc.generate_note", "actor-1");
        c.capabilities = vec!["note_generation".into()];
        c.properties.insert("patient.active".into(), "true".into());
        c.properties
            .insert("patient.deceased".into(), "false".into());

        let result = engine.evaluate_with_skill(&c).unwrap();
        assert!(result.spec_hash.is_some());
        assert_eq!(result.spec_hash.as_ref().unwrap().len(), 64);
    }

    #[test]
    fn evaluate_with_skill_no_role_in_context_skips_role_check() {
        let engine = skill_engine(SKILL_TOML);
        let mut c = ctx("ambient_doc.generate_note", "actor-1");
        c.capabilities = vec!["note_generation".into()];
        // role is None — should skip role check
        c.properties.insert("patient.active".into(), "true".into());
        c.properties
            .insert("patient.deceased".into(), "false".into());

        let result = engine.evaluate_with_skill(&c).unwrap();
        assert_eq!(result.decision, PolicyDecision::Allow);
    }

    #[test]
    fn load_skills_from_str_validates_audit() {
        let result = PolicyEngine::new().load_skills_from_str(
            r#"
            [[skill]]
            id = "bad.skill"
            name = "Bad"
            version = "1.0.0"
            severity = "low"
            action = "bad.action"

            [skill.audit]
            intent = "short"
            rationale = "ok rationale"

            [skill.audit.provenance]
            type = "expert"
            reference = "Internal"
        "#,
        );
        assert!(matches!(result, Err(PolicyError::InvalidRule(_))));
    }
}
