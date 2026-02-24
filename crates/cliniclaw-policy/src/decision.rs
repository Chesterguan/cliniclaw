#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    Allow,
    Deny,
    RequireApproval,
}

impl std::fmt::Display for PolicyDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyDecision::Allow => write!(f, "allow"),
            PolicyDecision::Deny => write!(f, "deny"),
            PolicyDecision::RequireApproval => write!(f, "require_approval"),
        }
    }
}
