//! VeritasGate: always computes the policy decision (the counterfactual).

use cliniclaw_policy::{ActionContext, PolicyDecision, PolicyEngine};

pub struct VeritasGate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateDecision {
    pub decision: PolicyDecision,
    pub skill_id: Option<String>,
    pub spec_hash: Option<String>,
}

impl VeritasGate {
    /// Evaluate; on any PolicyError, fail closed (Deny) — deny-by-default.
    pub fn evaluate(engine: &PolicyEngine, ctx: &ActionContext) -> GateDecision {
        match engine.evaluate_with_skill(ctx) {
            Ok(e) => GateDecision { decision: e.decision, skill_id: e.skill_id, spec_hash: e.spec_hash },
            Err(_) => GateDecision { decision: PolicyDecision::Deny, skill_id: None, spec_hash: None },
        }
    }
    /// Whether the action is applied in this arm: gate-on applies only on Allow;
    /// gate-off always applies (records the counterfactual).
    pub fn applies(decision: &PolicyDecision, gate_on: bool) -> bool {
        if !gate_on { return true; }
        matches!(decision, PolicyDecision::Allow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> PolicyEngine {
        let mut e = PolicyEngine::new();
        e.load_rego_str("order_entry.rego", r#"
package cliniclaw.order_entry
default decision := "deny"
decision := "allow" if {
    startswith(input.action, "order_entry.")
    "order_entry" in input.capabilities
}
"#).unwrap();
        e
    }

    #[test]
    fn computes_decision() {
        let e = engine();
        let mut ctx = ActionContext::new("order_entry.propose", "prac-1");
        ctx.capabilities = vec!["order_entry".into()];
        let g = VeritasGate::evaluate(&e, &ctx);
        assert_eq!(g.decision, PolicyDecision::Allow);
    }
    #[test]
    fn fails_closed_on_no_match() {
        let e = engine();
        let ctx = ActionContext::new("order_entry.propose", "prac-1");
        assert_eq!(VeritasGate::evaluate(&e, &ctx).decision, PolicyDecision::Deny);
    }
    #[test]
    fn apply_semantics() {
        assert!(!VeritasGate::applies(&PolicyDecision::Deny, true));
        assert!(VeritasGate::applies(&PolicyDecision::Deny, false));
        assert!(VeritasGate::applies(&PolicyDecision::Allow, true));
    }
}
