use tracing::{info, warn};

use crate::{context::ActionContext, decision::PolicyDecision, error::PolicyError};

/// Wraps a `regorus::Engine` to evaluate Rego policies for ClinicClaw.
///
/// All `.rego` policy files are loaded into a single regorus Engine instance.
/// Each evaluation clones the engine, sets the input from `ActionContext`, and
/// queries the matching package's `decision` rule.
#[derive(Debug, Clone)]
pub(crate) struct RegoEngine {
    engine: regorus::Engine,
}

impl RegoEngine {
    pub fn new() -> Self {
        let mut engine = regorus::Engine::new();
        engine.set_rego_v1(true);
        Self { engine }
    }

    /// Load a `.rego` policy file into the engine.
    pub fn add_policy_from_file(
        &mut self,
        path: &std::path::Path,
    ) -> Result<(), PolicyError> {
        self.engine
            .add_policy_from_file(path)
            .map_err(|e| PolicyError::LoadError(format!("rego load {}: {e}", path.display())))?;
        info!(path = %path.display(), "loaded rego policy");
        Ok(())
    }

    /// Load a rego policy from a string (for tests).
    pub fn add_policy(&mut self, name: &str, rego: &str) -> Result<(), PolicyError> {
        self.engine
            .add_policy(name.to_string(), rego.to_string())
            .map_err(|e| PolicyError::LoadError(format!("rego parse {name}: {e}")))?;
        Ok(())
    }

    /// Evaluate an `ActionContext` against loaded Rego policies.
    ///
    /// Determines the package from the action namespace (e.g. `ambient_doc.generate_note`
    /// → package `cliniclaw.ambient_doc`) and queries its `decision` rule.
    ///
    /// Returns `PolicyDecision::Deny` if:
    /// - No matching package exists
    /// - The `decision` rule evaluates to "deny" or an unrecognized value
    /// - An evaluation error occurs (deny-by-default on error)
    pub fn evaluate(&self, context: &ActionContext) -> PolicyDecision {
        match self.evaluate_inner(context) {
            Ok(decision) => decision,
            Err(e) => {
                warn!(
                    action = %context.action,
                    actor_id = %context.actor_id,
                    error = %e,
                    "rego evaluation error — deny-by-default"
                );
                PolicyDecision::Deny
            }
        }
    }

    fn evaluate_inner(&self, context: &ActionContext) -> Result<PolicyDecision, PolicyError> {
        let mut engine = self.engine.clone();

        // Serialize ActionContext to JSON for regorus input
        let input_json = serde_json::to_string(context)
            .map_err(|e| PolicyError::EvaluationError(format!("input serialization: {e}")))?;

        engine
            .set_input_json(&input_json)
            .map_err(|e| PolicyError::EvaluationError(format!("input parse: {e}")))?;

        // Determine which package to query based on action namespace
        let package = action_to_package(&context.action);
        let rule_path = format!("data.cliniclaw.{package}.decision");

        let result = engine
            .eval_rule(rule_path.clone())
            .map_err(|e| PolicyError::EvaluationError(format!("eval {rule_path}: {e}")))?;

        // Parse the result Value into PolicyDecision
        let decision = match result.as_string() {
            Ok(s) => match s.as_ref() {
                "allow" => PolicyDecision::Allow,
                "deny" => PolicyDecision::Deny,
                "require_approval" => PolicyDecision::RequireApproval,
                other => {
                    warn!(value = other, "unexpected rego decision value — treating as deny");
                    PolicyDecision::Deny
                }
            },
            Err(_) => {
                // Value is not a string (Undefined, Bool, etc.) — deny by default
                PolicyDecision::Deny
            }
        };

        info!(
            action = %context.action,
            actor_id = %context.actor_id,
            decision = %decision,
            "rego policy decision"
        );

        Ok(decision)
    }
}

/// Extract the package namespace from an action string.
///
/// `"ambient_doc.generate_note"` → `"ambient_doc"`
/// `"order_entry.propose_high_risk"` → `"order_entry"`
fn action_to_package(action: &str) -> &str {
    action.split('.').next().unwrap_or(action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_to_package_basic() {
        assert_eq!(action_to_package("ambient_doc.generate_note"), "ambient_doc");
        assert_eq!(action_to_package("order_entry.propose_high_risk"), "order_entry");
        assert_eq!(action_to_package("no_dot_action"), "no_dot_action");
    }

    fn make_context(action: &str, caps: &[&str]) -> ActionContext {
        let mut ctx = ActionContext::new(action, "test-actor");
        ctx.capabilities = caps.iter().map(|s| s.to_string()).collect();
        ctx
    }

    #[test]
    fn rego_deny_by_default() {
        let mut engine = RegoEngine::new();
        engine
            .add_policy(
                "test.rego",
                r#"
package cliniclaw.test_domain

default decision := "deny"
"#,
            )
            .unwrap();

        let ctx = make_context("test_domain.some_action", &[]);
        assert_eq!(engine.evaluate(&ctx), PolicyDecision::Deny);
    }

    #[test]
    fn rego_allow_with_capability() {
        let mut engine = RegoEngine::new();
        engine
            .add_policy(
                "test.rego",
                r#"
package cliniclaw.test_domain

default decision := "deny"

decision := "allow" if {
    input.action == "test_domain.do_thing"
    "test_cap" in input.capabilities
}
"#,
            )
            .unwrap();

        let ctx = make_context("test_domain.do_thing", &["test_cap"]);
        assert_eq!(engine.evaluate(&ctx), PolicyDecision::Allow);
    }

    #[test]
    fn rego_deny_missing_capability() {
        let mut engine = RegoEngine::new();
        engine
            .add_policy(
                "test.rego",
                r#"
package cliniclaw.test_domain

default decision := "deny"

decision := "allow" if {
    input.action == "test_domain.do_thing"
    "test_cap" in input.capabilities
}
"#,
            )
            .unwrap();

        let ctx = make_context("test_domain.do_thing", &["wrong_cap"]);
        assert_eq!(engine.evaluate(&ctx), PolicyDecision::Deny);
    }

    #[test]
    fn rego_require_approval() {
        let mut engine = RegoEngine::new();
        engine
            .add_policy(
                "test.rego",
                r#"
package cliniclaw.test_domain

default decision := "deny"

decision := "require_approval" if {
    input.action == "test_domain.high_risk"
    "test_cap" in input.capabilities
}
"#,
            )
            .unwrap();

        let ctx = make_context("test_domain.high_risk", &["test_cap"]);
        assert_eq!(engine.evaluate(&ctx), PolicyDecision::RequireApproval);
    }

    #[test]
    fn rego_condition_on_properties() {
        let mut engine = RegoEngine::new();
        engine
            .add_policy(
                "test.rego",
                r#"
package cliniclaw.test_domain

default decision := "deny"

decision := "allow" if {
    input.action == "test_domain.do_thing"
    "test_cap" in input.capabilities
    input.properties.encounter_status == "in-progress"
}
"#,
            )
            .unwrap();

        let mut ctx = make_context("test_domain.do_thing", &["test_cap"]);
        ctx.properties
            .insert("encounter_status".to_string(), "in-progress".to_string());
        assert_eq!(engine.evaluate(&ctx), PolicyDecision::Allow);

        // Wrong status → deny
        let mut ctx2 = make_context("test_domain.do_thing", &["test_cap"]);
        ctx2.properties
            .insert("encounter_status".to_string(), "finished".to_string());
        assert_eq!(engine.evaluate(&ctx2), PolicyDecision::Deny);
    }

    #[test]
    fn rego_no_matching_package_denies() {
        let engine = RegoEngine::new();
        let ctx = make_context("nonexistent.action", &[]);
        assert_eq!(engine.evaluate(&ctx), PolicyDecision::Deny);
    }
}
