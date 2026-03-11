use tracing::{info, warn};

use crate::{
    context::ActionContext,
    decision::PolicyDecision,
    error::PolicyError,
    rego_engine::RegoEngine,
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

/// Policy engine backed by OPA Rego (via regorus) for rule evaluation
/// and TOML-based clinical skill metadata.
///
/// Rule evaluation is handled by Rego policies. Skill metadata (audit
/// provenance, population gates, severity escalation) remains in Rust/TOML.
#[derive(Debug, Clone)]
pub struct PolicyEngine {
    rego: RegoEngine,
    skills: Vec<ClinicalSkillSpec>,
}

impl PolicyEngine {
    pub fn new() -> Self {
        Self {
            rego: RegoEngine::new(),
            skills: Vec::new(),
        }
    }

    /// Load a Rego policy from a string.
    ///
    /// `name` is a logical filename for error messages (e.g. "ambient_doc.rego").
    pub fn load_rego_str(&mut self, name: &str, rego: &str) -> Result<(), PolicyError> {
        self.rego.add_policy(name, rego)
    }

    /// Load a `.rego` policy file into the engine.
    pub fn load_rego_file(&mut self, path: impl AsRef<std::path::Path>) -> Result<(), PolicyError> {
        self.rego.add_policy_from_file(path.as_ref())
    }

    /// Load clinical skill specs from a TOML string.
    ///
    /// Only `[[skill]]` sections are loaded. Any `[[rule]]` sections in the
    /// TOML are silently ignored — rules must be provided via `.rego` files.
    pub fn load_skills_from_str(&mut self, toml_str: &str) -> Result<(), PolicyError> {
        let file: SkillPolicyFile =
            toml::from_str(toml_str).map_err(|e| PolicyError::LoadError(e.to_string()))?;

        for skill in &file.skills {
            skill.validate()?;
        }

        let skill_count = file.skills.len();

        let mut skills = file.skills;
        for skill in &mut skills {
            skill.spec_hash = skill.compute_spec_hash();
        }
        self.skills.extend(skills);

        info!(
            loaded_skills = skill_count,
            total_skills = self.skills.len(),
            "clinical skills loaded"
        );
        Ok(())
    }

    /// Load clinical skill specs from a TOML file.
    ///
    /// Only `[[skill]]` sections are loaded. Any `[[rule]]` sections are
    /// silently ignored — rules must come from `.rego` files.
    pub fn load_skills_from_file(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), PolicyError> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .map_err(|e| PolicyError::LoadError(format!("{}: {}", path.display(), e)))?;
        self.load_skills_from_str(&contents)
    }

    /// Load all policy files from a directory.
    ///
    /// - `.rego` files → Rego policy rules
    /// - `.toml` files → Clinical skill metadata
    pub fn load_policies_dir(
        &mut self,
        dir: impl AsRef<std::path::Path>,
    ) -> Result<(), PolicyError> {
        let dir = dir.as_ref();
        if !dir.exists() {
            return Err(PolicyError::LoadError(format!(
                "policy directory not found: {}",
                dir.display()
            )));
        }

        let entries = std::fs::read_dir(dir)
            .map_err(|e| PolicyError::LoadError(format!("{}: {}", dir.display(), e)))?;

        for entry in entries {
            let entry =
                entry.map_err(|e| PolicyError::LoadError(format!("{}: {}", dir.display(), e)))?;
            let path = entry.path();

            match path.extension().and_then(|ext| ext.to_str()) {
                Some("rego") => {
                    info!(path = %path.display(), "loading rego policy");
                    self.load_rego_file(&path)?;
                }
                Some("toml") => {
                    info!(path = %path.display(), "loading skill definitions");
                    self.load_skills_from_file(&path)?;
                }
                _ => {} // skip non-policy files
            }
        }

        Ok(())
    }

    /// Evaluate an action context against loaded Rego policies.
    ///
    /// Deny-by-default: if no Rego package matches or evaluation fails,
    /// the action is denied.
    pub fn evaluate(&self, context: &ActionContext) -> PolicyDecision {
        self.rego.evaluate(context)
    }

    /// Skill-aware evaluation. Checks skill metadata (role, capability tokens,
    /// population) before falling through to Rego policy rule evaluation.
    ///
    /// Order: skill lookup → role check → capability token validation →
    /// population gate → Rego policy evaluation → severity escalation.
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

                // Step 4: Rego policy rule evaluation
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
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ActionContext;
    use crate::decision::PolicyDecision;

    /// Create an engine with inline Rego rules.
    fn engine_with_rego(name: &str, rego: &str) -> PolicyEngine {
        let mut engine = PolicyEngine::new();
        engine.load_rego_str(name, rego).expect("valid Rego");
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

    // ── Core evaluation tests ──────────────────────────────────────

    #[test]
    fn deny_by_default_no_rules() {
        let engine = PolicyEngine::new();
        let context = ctx("ambient_doc.generate_note", "practitioner-1");
        assert_eq!(engine.evaluate(&context), PolicyDecision::Deny);
    }

    #[test]
    fn allow_with_matching_capability() {
        let engine = engine_with_rego(
            "ambient_doc.rego",
            r#"
package cliniclaw.ambient_doc

default decision := "deny"

decision := "allow" if {
    input.action == "ambient_doc.generate_note"
    "note_generation" in input.capabilities
}
"#,
        );
        let mut context = ctx("ambient_doc.generate_note", "practitioner-1");
        context.capabilities = vec!["note_generation".to_string()];
        assert_eq!(engine.evaluate(&context), PolicyDecision::Allow);
    }

    #[test]
    fn deny_missing_required_capability() {
        let engine = engine_with_rego(
            "ambient_doc.rego",
            r#"
package cliniclaw.ambient_doc

default decision := "deny"

decision := "allow" if {
    input.action == "ambient_doc.generate_note"
    "note_generation" in input.capabilities
}
"#,
        );
        let context = ctx("ambient_doc.generate_note", "practitioner-1");
        assert_eq!(engine.evaluate(&context), PolicyDecision::Deny);
    }

    #[test]
    fn wildcard_action_matches_namespace() {
        let engine = engine_with_rego(
            "order_entry.rego",
            r#"
package cliniclaw.order_entry

default decision := "deny"

decision := "allow" if {
    startswith(input.action, "order_entry.")
    "order_entry" in input.capabilities
}
"#,
        );
        let context = ctx_with_caps("order_entry.propose", "practitioner-1", &["order_entry"]);
        assert_eq!(engine.evaluate(&context), PolicyDecision::Allow);

        let context2 = ctx_with_caps("order_entry.review", "practitioner-1", &["order_entry"]);
        assert_eq!(engine.evaluate(&context2), PolicyDecision::Allow);
    }

    #[test]
    fn wildcard_does_not_match_different_namespace() {
        let engine = engine_with_rego(
            "order_entry.rego",
            r#"
package cliniclaw.order_entry

default decision := "deny"

decision := "allow" if {
    startswith(input.action, "order_entry.")
    "order_entry" in input.capabilities
}
"#,
        );
        let context =
            ctx_with_caps("ambient_doc.generate_note", "practitioner-1", &["order_entry"]);
        assert_eq!(engine.evaluate(&context), PolicyDecision::Deny);
    }

    #[test]
    fn higher_priority_deny_wins() {
        // In Rego, deny and allow are mutually exclusive via conditions.
        // The deny rule matches "finished" encounters specifically, preventing
        // the allow rule from firing.
        let engine = engine_with_rego(
            "ambient_doc.rego",
            r#"
package cliniclaw.ambient_doc

default decision := "deny"

decision := "deny" if {
    input.action == "ambient_doc.generate_note"
    input.properties.encounter_status == "finished"
}

decision := "allow" if {
    input.action == "ambient_doc.generate_note"
    "note_generation" in input.capabilities
    input.properties.encounter_status == "in-progress"
}
"#,
        );
        // Finished encounter → deny wins
        let mut context =
            ctx_with_caps("ambient_doc.generate_note", "practitioner-1", &["note_generation"]);
        context
            .properties
            .insert("encounter_status".to_string(), "finished".to_string());
        assert_eq!(engine.evaluate(&context), PolicyDecision::Deny);
    }

    #[test]
    fn require_approval_decision() {
        let engine = engine_with_rego(
            "order_entry.rego",
            r#"
package cliniclaw.order_entry

default decision := "deny"

decision := "require_approval" if {
    input.action == "order_entry.propose_high_risk"
    "order_entry" in input.capabilities
}
"#,
        );
        let context =
            ctx_with_caps("order_entry.propose_high_risk", "practitioner-1", &["order_entry"]);
        assert_eq!(engine.evaluate(&context), PolicyDecision::RequireApproval);
    }

    #[test]
    fn condition_match_allows_action() {
        let engine = engine_with_rego(
            "ambient_doc.rego",
            r#"
package cliniclaw.ambient_doc

default decision := "deny"

decision := "allow" if {
    input.action == "ambient_doc.generate_note"
    "note_generation" in input.capabilities
    input.properties.encounter_status == "in-progress"
}
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
        let engine = engine_with_rego(
            "ambient_doc.rego",
            r#"
package cliniclaw.ambient_doc

default decision := "deny"

decision := "allow" if {
    input.action == "ambient_doc.generate_note"
    "note_generation" in input.capabilities
    input.properties.encounter_status == "in-progress"
}
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
        let engine = engine_with_rego(
            "ambient_doc.rego",
            r#"
package cliniclaw.ambient_doc

default decision := "deny"

decision := "allow" if {
    input.action == "ambient_doc.generate_note"
    "note_generation" in input.capabilities
    input.properties.encounter_status == "in-progress"
}
"#,
        );
        let context =
            ctx_with_caps("ambient_doc.generate_note", "practitioner-1", &["note_generation"]);
        assert_eq!(engine.evaluate(&context), PolicyDecision::Deny);
    }

    #[test]
    fn invalid_rego_returns_load_error() {
        let mut engine = PolicyEngine::new();
        let result = engine.load_rego_str("bad.rego", "this is not valid rego !!!");
        assert!(matches!(result, Err(PolicyError::LoadError(_))));
    }

    // ── Skill-aware evaluation tests ──────────────────────────────

    /// Create an engine with both Rego rules and TOML skill specs.
    fn skill_engine(rego_name: &str, rego: &str, skill_toml: &str) -> PolicyEngine {
        let mut engine = PolicyEngine::new();
        engine.load_rego_str(rego_name, rego).expect("valid Rego");
        engine
            .load_skills_from_str(skill_toml)
            .expect("valid skill TOML");
        engine
    }

    const SKILL_REGO: &str = r#"
package cliniclaw.ambient_doc

default decision := "deny"

decision := "allow" if {
    input.action == "ambient_doc.generate_note"
    "note_generation" in input.capabilities
}
"#;

    const SKILL_TOML: &str = r#"
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
        let engine = skill_engine("ambient_doc.rego", SKILL_REGO, SKILL_TOML);
        let mut c = ctx("ambient_doc.generate_note", "actor-1");
        c.capabilities = vec!["note_generation".into()];
        c.role = Some("receptionist".into());

        let result = engine.evaluate_with_skill(&c);
        assert!(matches!(result, Err(PolicyError::RoleNotAllowed { .. })));
    }

    #[test]
    fn evaluate_with_skill_role_allowed() {
        let engine = skill_engine("ambient_doc.rego", SKILL_REGO, SKILL_TOML);
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
        let engine = skill_engine("ambient_doc.rego", SKILL_REGO, SKILL_TOML);
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
        let engine = skill_engine("ambient_doc.rego", SKILL_REGO, SKILL_TOML);
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
        let rego = r#"
package cliniclaw.order_entry

default decision := "deny"

decision := "allow" if {
    input.action == "order_entry.propose_controlled"
    "order_entry" in input.capabilities
}
"#;
        let skill_toml = r#"
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
        "#;

        let engine = skill_engine("order_entry.rego", rego, skill_toml);
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
        // Only Rego rules loaded, no skills
        let engine = engine_with_rego(
            "ambient_doc.rego",
            r#"
package cliniclaw.ambient_doc

default decision := "deny"

decision := "allow" if {
    input.action == "ambient_doc.generate_note"
    "note_generation" in input.capabilities
}
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
        let engine = skill_engine("ambient_doc.rego", SKILL_REGO, SKILL_TOML);
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
        let engine = skill_engine("ambient_doc.rego", SKILL_REGO, SKILL_TOML);
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

    // ── Rego-specific tests (new capabilities TOML couldn't express) ──

    #[test]
    fn rego_or_logic_via_set_membership() {
        // Nurse assessment allows multiple encounter statuses — something
        // that required 3 separate TOML rules now expressed as one Rego rule.
        let engine = engine_with_rego(
            "nurse_assess.rego",
            r#"
package cliniclaw.nurse_assess

default decision := "deny"

decision := "allow" if {
    input.action == "nurse_assess.evaluate"
    "nurse_assess" in input.capabilities
    input.properties.encounter_status in {"in-progress", "arrived", "triaged"}
}
"#,
        );

        for status in &["in-progress", "arrived", "triaged"] {
            let mut c = ctx_with_caps("nurse_assess.evaluate", "nurse-1", &["nurse_assess"]);
            c.properties
                .insert("encounter_status".to_string(), status.to_string());
            assert_eq!(
                engine.evaluate(&c),
                PolicyDecision::Allow,
                "should allow for status {status}"
            );
        }

        // "finished" should deny
        let mut c = ctx_with_caps("nurse_assess.evaluate", "nurse-1", &["nurse_assess"]);
        c.properties
            .insert("encounter_status".to_string(), "finished".to_string());
        assert_eq!(engine.evaluate(&c), PolicyDecision::Deny);
    }

    #[test]
    fn rego_negation_for_discharge_plan() {
        // Discharge plan: inpatient requires approval, others allowed.
        // Uses `not` — impossible in old TOML DSL.
        let engine = engine_with_rego(
            "discharge_plan.rego",
            r#"
package cliniclaw.discharge_plan

default decision := "deny"

decision := "deny" if {
    input.action == "discharge_plan.generate"
    input.properties.encounter_status == "finished"
}

decision := "require_approval" if {
    input.action == "discharge_plan.generate"
    "discharge_plan" in input.capabilities
    input.properties.encounter_status == "in-progress"
    input.properties.encounter_class == "IMP"
}

decision := "allow" if {
    input.action == "discharge_plan.generate"
    "discharge_plan" in input.capabilities
    input.properties.encounter_status == "in-progress"
    not input.properties.encounter_class == "IMP"
}
"#,
        );

        // Inpatient → require_approval
        let mut c =
            ctx_with_caps("discharge_plan.generate", "doc-1", &["discharge_plan"]);
        c.properties
            .insert("encounter_status".to_string(), "in-progress".to_string());
        c.properties
            .insert("encounter_class".to_string(), "IMP".to_string());
        assert_eq!(engine.evaluate(&c), PolicyDecision::RequireApproval);

        // Outpatient → allow
        let mut c2 =
            ctx_with_caps("discharge_plan.generate", "doc-1", &["discharge_plan"]);
        c2.properties
            .insert("encounter_status".to_string(), "in-progress".to_string());
        c2.properties
            .insert("encounter_class".to_string(), "AMB".to_string());
        assert_eq!(engine.evaluate(&c2), PolicyDecision::Allow);

        // No encounter_class → allow (not IMP)
        let mut c3 =
            ctx_with_caps("discharge_plan.generate", "doc-1", &["discharge_plan"]);
        c3.properties
            .insert("encounter_status".to_string(), "in-progress".to_string());
        assert_eq!(engine.evaluate(&c3), PolicyDecision::Allow);

        // Finished → deny
        let mut c4 =
            ctx_with_caps("discharge_plan.generate", "doc-1", &["discharge_plan"]);
        c4.properties
            .insert("encounter_status".to_string(), "finished".to_string());
        assert_eq!(engine.evaluate(&c4), PolicyDecision::Deny);
    }
}
