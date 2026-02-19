use std::collections::HashMap;

use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};

use crate::error::PolicyError;

/// Clinical severity level, inspired by PSDL.
/// Maps to VERITAS escalation behavior:
///   Critical → RequireApproval (always)
///   High     → logged with emphasis
///   Medium/Low → normal flow
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// The type of evidence backing a clinical skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceType {
    Guideline,
    Publication,
    Sop,
    Regulatory,
    Expert,
}

/// Evidence provenance for a clinical skill definition.
/// Adapted from PSDL's structured provenance format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    #[serde(rename = "type")]
    pub type_: ProvenanceType,
    pub reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Mandatory audit metadata for a clinical skill.
/// Answers WHO/WHY/WHAT for every skill definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillAudit {
    /// What the skill does clinically. Minimum 10 characters.
    pub intent: String,
    /// Why this skill matters clinically.
    pub rationale: String,
    /// What evidence supports this skill definition.
    pub provenance: Provenance,
}

impl SkillAudit {
    pub fn validate(&self) -> Result<(), PolicyError> {
        if self.intent.len() < 10 {
            return Err(PolicyError::InvalidRule(
                "skill audit.intent must be at least 10 characters".into(),
            ));
        }
        if self.rationale.is_empty() {
            return Err(PolicyError::InvalidRule(
                "skill audit.rationale must not be empty".into(),
            ));
        }
        if self.provenance.reference.is_empty() {
            return Err(PolicyError::InvalidRule(
                "skill audit.provenance.reference must not be empty".into(),
            ));
        }
        Ok(())
    }
}

/// Comparison operator for population criteria.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CriterionOp {
    Eq,
    NotEq,
    Gt,
    Lt,
    Gte,
    Lte,
}

/// A single population criterion parsed from a string like "encounter.status == in-progress".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopulationCriterion {
    pub field: String,
    pub op: CriterionOp,
    pub value: String,
}

impl PopulationCriterion {
    /// Parse a criterion string. Supported operators: ==, !=, >=, <=, >, <
    pub fn parse(s: &str) -> Result<Self, PolicyError> {
        // Two-char operators first to avoid partial matches
        let ops = [
            ("==", CriterionOp::Eq),
            ("!=", CriterionOp::NotEq),
            (">=", CriterionOp::Gte),
            ("<=", CriterionOp::Lte),
            (">", CriterionOp::Gt),
            ("<", CriterionOp::Lt),
        ];

        for (token, op) in ops {
            if let Some((left, right)) = s.split_once(token) {
                let field = left.trim().to_string();
                let value = right.trim().to_string();
                if field.is_empty() || value.is_empty() {
                    return Err(PolicyError::InvalidRule(format!(
                        "invalid population criterion: '{s}'"
                    )));
                }
                return Ok(Self { field, op, value });
            }
        }

        Err(PolicyError::InvalidRule(format!(
            "no valid operator found in population criterion: '{s}'"
        )))
    }

    /// Evaluate this criterion against a properties map.
    pub fn evaluate(&self, properties: &HashMap<String, String>) -> bool {
        match properties.get(&self.field) {
            None => false,
            Some(actual) => match self.op {
                CriterionOp::Eq => actual == &self.value,
                CriterionOp::NotEq => actual != &self.value,
                CriterionOp::Gt | CriterionOp::Lt | CriterionOp::Gte | CriterionOp::Lte => {
                    match (actual.parse::<f64>(), self.value.parse::<f64>()) {
                        (Ok(a), Ok(v)) => match self.op {
                            CriterionOp::Gt => a > v,
                            CriterionOp::Lt => a < v,
                            CriterionOp::Gte => a >= v,
                            CriterionOp::Lte => a <= v,
                            _ => unreachable!(),
                        },
                        _ => match self.op {
                            CriterionOp::Gt => actual.as_str() > self.value.as_str(),
                            CriterionOp::Lt => (actual.as_str()) < self.value.as_str(),
                            CriterionOp::Gte => actual.as_str() >= self.value.as_str(),
                            CriterionOp::Lte => actual.as_str() <= self.value.as_str(),
                            _ => unreachable!(),
                        },
                    }
                }
            },
        }
    }
}

/// Population gate: determines which patients/encounters a skill applies to.
/// Adapted from PSDL population scoping:
///   - include: ALL criteria must match (AND logic)
///   - exclude: ANY criterion match blocks (OR logic)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopulationGate {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl PopulationGate {
    /// Evaluate the population gate against FHIR-derived properties.
    pub fn evaluate(&self, properties: &HashMap<String, String>) -> Result<(), PolicyError> {
        for criterion_str in &self.include {
            let criterion = PopulationCriterion::parse(criterion_str)?;
            if !criterion.evaluate(properties) {
                return Err(PolicyError::PopulationExcluded {
                    reason: format!("inclusion criterion not met: '{criterion_str}'"),
                });
            }
        }

        for criterion_str in &self.exclude {
            let criterion = PopulationCriterion::parse(criterion_str)?;
            if criterion.evaluate(properties) {
                return Err(PolicyError::PopulationExcluded {
                    reason: format!("exclusion criterion matched: '{criterion_str}'"),
                });
            }
        }

        Ok(())
    }
}

/// A PSDL-inspired clinical skill specification.
///
/// Declares WHAT the skill does (audit, population, severity) but not
/// HOW it executes (that remains in the agent code).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClinicalSkillSpec {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub version: String,
    pub severity: Severity,
    #[serde(default)]
    pub allowed_roles: Vec<String>,
    pub action: String,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    pub audit: SkillAudit,
    #[serde(default)]
    pub population: Option<PopulationGate>,
    /// Computed at load time — SHA-256 of canonical skill definition
    #[serde(skip)]
    pub spec_hash: String,
}

impl ClinicalSkillSpec {
    pub fn validate(&self) -> Result<(), PolicyError> {
        if self.id.is_empty() {
            return Err(PolicyError::InvalidRule("skill id must not be empty".into()));
        }
        if self.name.is_empty() {
            return Err(PolicyError::InvalidRule("skill name must not be empty".into()));
        }
        if self.version.is_empty() {
            return Err(PolicyError::InvalidRule("skill version must not be empty".into()));
        }
        if self.action.is_empty() {
            return Err(PolicyError::InvalidRule("skill action must not be empty".into()));
        }
        self.audit.validate()?;
        Ok(())
    }

    /// Compute the canonical SHA-256 hash of this skill definition.
    /// Excludes `description` per PSDL hashing spec.
    pub fn compute_spec_hash(&self) -> String {
        let canonical = serde_json::json!({
            "id": self.id,
            "name": self.name,
            "version": self.version,
            "severity": self.severity,
            "allowed_roles": self.allowed_roles,
            "action": self.action,
            "required_capabilities": self.required_capabilities,
            "audit": {
                "intent": self.audit.intent,
                "rationale": self.audit.rationale,
                "provenance": {
                    "type": self.audit.provenance.type_,
                    "reference": self.audit.provenance.reference,
                    "uri": self.audit.provenance.uri,
                    "version": self.audit.provenance.version,
                }
            },
            "population": self.population,
        });

        let canonical_str =
            serde_json::to_string(&canonical).expect("canonical serialization should not fail");

        let mut hasher = Sha256::new();
        hasher.update(canonical_str.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Check if a given role is allowed to invoke this skill.
    /// Empty `allowed_roles` means any role is allowed (backward compat).
    pub fn is_role_allowed(&self, role: &str) -> bool {
        if self.allowed_roles.is_empty() {
            return true;
        }
        self.allowed_roles.iter().any(|r| r == role)
    }

    /// Evaluate the population gate. If no gate is defined, passes unconditionally.
    pub fn check_population(
        &self,
        properties: &HashMap<String, String>,
    ) -> Result<(), PolicyError> {
        if let Some(ref gate) = self.population {
            gate.evaluate(properties)?;
        }
        Ok(())
    }
}

/// TOML deserialization container that supports both [[rule]] and [[skill]].
#[derive(Debug, Clone, Deserialize)]
pub struct SkillPolicyFile {
    #[serde(rename = "rule", default)]
    pub rules: Vec<crate::rule::PolicyRule>,
    #[serde(rename = "skill", default)]
    pub skills: Vec<ClinicalSkillSpec>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── PopulationCriterion parsing ─────────────────────────────────

    #[test]
    fn parse_eq_criterion() {
        let c = PopulationCriterion::parse("encounter.status == in-progress").unwrap();
        assert_eq!(c.field, "encounter.status");
        assert_eq!(c.op, CriterionOp::Eq);
        assert_eq!(c.value, "in-progress");
    }

    #[test]
    fn parse_neq_criterion() {
        let c = PopulationCriterion::parse("patient.deceased != true").unwrap();
        assert_eq!(c.field, "patient.deceased");
        assert_eq!(c.op, CriterionOp::NotEq);
        assert_eq!(c.value, "true");
    }

    #[test]
    fn parse_gte_criterion() {
        let c = PopulationCriterion::parse("patient.age >= 18").unwrap();
        assert_eq!(c.field, "patient.age");
        assert_eq!(c.op, CriterionOp::Gte);
        assert_eq!(c.value, "18");
    }

    #[test]
    fn parse_lt_criterion() {
        let c = PopulationCriterion::parse("patient.age < 65").unwrap();
        assert_eq!(c.field, "patient.age");
        assert_eq!(c.op, CriterionOp::Lt);
        assert_eq!(c.value, "65");
    }

    #[test]
    fn parse_invalid_no_operator() {
        assert!(PopulationCriterion::parse("no operator here").is_err());
    }

    #[test]
    fn parse_invalid_empty_field() {
        assert!(PopulationCriterion::parse("== value_only").is_err());
    }

    #[test]
    fn parse_invalid_empty_value() {
        assert!(PopulationCriterion::parse("field_only ==").is_err());
    }

    // ── PopulationCriterion evaluation ──────────────────────────────

    #[test]
    fn evaluate_eq_true() {
        let c = PopulationCriterion::parse("encounter.status == in-progress").unwrap();
        let mut props = HashMap::new();
        props.insert("encounter.status".into(), "in-progress".into());
        assert!(c.evaluate(&props));
    }

    #[test]
    fn evaluate_eq_false() {
        let c = PopulationCriterion::parse("encounter.status == in-progress").unwrap();
        let mut props = HashMap::new();
        props.insert("encounter.status".into(), "finished".into());
        assert!(!c.evaluate(&props));
    }

    #[test]
    fn evaluate_missing_property_returns_false() {
        let c = PopulationCriterion::parse("encounter.status == in-progress").unwrap();
        let props = HashMap::new();
        assert!(!c.evaluate(&props));
    }

    #[test]
    fn evaluate_numeric_gte() {
        let c = PopulationCriterion::parse("patient.age >= 18").unwrap();
        let mut props = HashMap::new();

        props.insert("patient.age".into(), "42".into());
        assert!(c.evaluate(&props));

        props.insert("patient.age".into(), "17".into());
        assert!(!c.evaluate(&props));

        props.insert("patient.age".into(), "18".into());
        assert!(c.evaluate(&props));
    }

    #[test]
    fn evaluate_neq() {
        let c = PopulationCriterion::parse("patient.deceased != true").unwrap();
        let mut props = HashMap::new();
        props.insert("patient.deceased".into(), "false".into());
        assert!(c.evaluate(&props));

        props.insert("patient.deceased".into(), "true".into());
        assert!(!c.evaluate(&props));
    }

    // ── PopulationGate ──────────────────────────────────────────────

    #[test]
    fn population_gate_include_all_pass() {
        let gate = PopulationGate {
            include: vec![
                "encounter.status == in-progress".into(),
                "patient.active == true".into(),
            ],
            exclude: vec![],
        };
        let mut props = HashMap::new();
        props.insert("encounter.status".into(), "in-progress".into());
        props.insert("patient.active".into(), "true".into());
        assert!(gate.evaluate(&props).is_ok());
    }

    #[test]
    fn population_gate_include_one_fails() {
        let gate = PopulationGate {
            include: vec![
                "encounter.status == in-progress".into(),
                "patient.active == true".into(),
            ],
            exclude: vec![],
        };
        let mut props = HashMap::new();
        props.insert("encounter.status".into(), "finished".into());
        props.insert("patient.active".into(), "true".into());
        assert!(matches!(
            gate.evaluate(&props),
            Err(PolicyError::PopulationExcluded { .. })
        ));
    }

    #[test]
    fn population_gate_exclude_any_blocks() {
        let gate = PopulationGate {
            include: vec!["patient.active == true".into()],
            exclude: vec![
                "patient.deceased == true".into(),
                "encounter.class == emergency".into(),
            ],
        };
        let mut props = HashMap::new();
        props.insert("patient.active".into(), "true".into());
        props.insert("patient.deceased".into(), "false".into());
        props.insert("encounter.class".into(), "emergency".into());
        assert!(matches!(
            gate.evaluate(&props),
            Err(PolicyError::PopulationExcluded { .. })
        ));
    }

    #[test]
    fn population_gate_exclude_none_match_passes() {
        let gate = PopulationGate {
            include: vec!["patient.active == true".into()],
            exclude: vec!["patient.deceased == true".into()],
        };
        let mut props = HashMap::new();
        props.insert("patient.active".into(), "true".into());
        props.insert("patient.deceased".into(), "false".into());
        assert!(gate.evaluate(&props).is_ok());
    }

    #[test]
    fn population_gate_empty_passes() {
        let gate = PopulationGate {
            include: vec![],
            exclude: vec![],
        };
        assert!(gate.evaluate(&HashMap::new()).is_ok());
    }

    // ── SkillAudit validation ───────────────────────────────────────

    #[test]
    fn audit_intent_too_short() {
        let audit = SkillAudit {
            intent: "short".into(),
            rationale: "some rationale".into(),
            provenance: Provenance {
                type_: ProvenanceType::Sop,
                reference: "SOP v1".into(),
                uri: None,
                version: None,
            },
        };
        assert!(matches!(audit.validate(), Err(PolicyError::InvalidRule(_))));
    }

    #[test]
    fn audit_empty_rationale() {
        let audit = SkillAudit {
            intent: "Generate clinical notes from encounters".into(),
            rationale: String::new(),
            provenance: Provenance {
                type_: ProvenanceType::Sop,
                reference: "SOP v1".into(),
                uri: None,
                version: None,
            },
        };
        assert!(matches!(audit.validate(), Err(PolicyError::InvalidRule(_))));
    }

    #[test]
    fn audit_valid() {
        let audit = SkillAudit {
            intent: "Generate structured SOAP notes from encounter transcripts".into(),
            rationale: "Reduces documentation burden".into(),
            provenance: Provenance {
                type_: ProvenanceType::Sop,
                reference: "Clinical Documentation SOP v1.0".into(),
                uri: Some("https://example.com/sop".into()),
                version: Some("1.0".into()),
            },
        };
        assert!(audit.validate().is_ok());
    }

    // ── Spec hash ───────────────────────────────────────────────────

    #[test]
    fn spec_hash_is_deterministic() {
        let skill = test_skill();
        let hash1 = skill.compute_spec_hash();
        let hash2 = skill.compute_spec_hash();
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn spec_hash_excludes_description() {
        let mut skill1 = test_skill();
        skill1.description = Some("first description".into());
        let mut skill2 = test_skill();
        skill2.description = Some("completely different description".into());
        assert_eq!(skill1.compute_spec_hash(), skill2.compute_spec_hash());
    }

    #[test]
    fn spec_hash_changes_with_intent() {
        let skill1 = test_skill();
        let mut skill2 = test_skill();
        skill2.audit.intent = "Different intent that changes the hash value".into();
        assert_ne!(skill1.compute_spec_hash(), skill2.compute_spec_hash());
    }

    #[test]
    fn spec_hash_changes_with_version() {
        let skill1 = test_skill();
        let mut skill2 = test_skill();
        skill2.version = "2.0.0".into();
        assert_ne!(skill1.compute_spec_hash(), skill2.compute_spec_hash());
    }

    // ── Role check ──────────────────────────────────────────────────

    #[test]
    fn role_allowed_when_in_list() {
        let skill = test_skill();
        assert!(skill.is_role_allowed("physician"));
        assert!(skill.is_role_allowed("nurse_practitioner"));
    }

    #[test]
    fn role_denied_when_not_in_list() {
        let skill = test_skill();
        assert!(!skill.is_role_allowed("receptionist"));
    }

    #[test]
    fn role_allowed_when_list_empty() {
        let mut skill = test_skill();
        skill.allowed_roles = vec![];
        assert!(skill.is_role_allowed("anyone"));
    }

    // ── ClinicalSkillSpec validation ────────────────────────────────

    #[test]
    fn skill_validate_empty_id() {
        let mut skill = test_skill();
        skill.id = String::new();
        assert!(skill.validate().is_err());
    }

    #[test]
    fn skill_validate_valid() {
        let skill = test_skill();
        assert!(skill.validate().is_ok());
    }

    // ── TOML deserialization ────────────────────────────────────────

    #[test]
    fn deserialize_skill_from_toml() {
        let toml_str = r#"
            [[skill]]
            id = "ambient_doc.generate_note"
            name = "Ambient Documentation"
            version = "1.0.0"
            severity = "medium"
            allowed_roles = ["physician"]
            action = "ambient_doc.generate_note"
            required_capabilities = ["note_generation"]

            [skill.audit]
            intent = "Generate structured SOAP notes from encounter transcripts"
            rationale = "Reduces clinician documentation burden"

            [skill.audit.provenance]
            type = "sop"
            reference = "ClinicClaw SOP v1"
        "#;

        let file: SkillPolicyFile = toml::from_str(toml_str).unwrap();
        assert_eq!(file.skills.len(), 1);
        assert_eq!(file.skills[0].id, "ambient_doc.generate_note");
        assert_eq!(file.skills[0].severity, Severity::Medium);
        assert_eq!(file.skills[0].audit.provenance.type_, ProvenanceType::Sop);
    }

    #[test]
    fn deserialize_mixed_rules_and_skills() {
        let toml_str = r#"
            [[rule]]
            name = "allow_note_gen"
            action = "ambient_doc.generate_note"
            decision = "allow"
            required_capabilities = ["note_generation"]
            priority = 10

            [[skill]]
            id = "ambient_doc.generate_note"
            name = "Ambient Documentation"
            version = "1.0.0"
            severity = "medium"
            action = "ambient_doc.generate_note"

            [skill.audit]
            intent = "Generate clinical notes from transcripts"
            rationale = "Documentation efficiency"

            [skill.audit.provenance]
            type = "sop"
            reference = "SOP v1"
        "#;

        let file: SkillPolicyFile = toml::from_str(toml_str).unwrap();
        assert_eq!(file.rules.len(), 1);
        assert_eq!(file.skills.len(), 1);
    }

    #[test]
    fn deserialize_skill_with_population() {
        let toml_str = r#"
            [[skill]]
            id = "test.skill"
            name = "Test Skill"
            version = "1.0.0"
            severity = "high"
            action = "test.action"

            [skill.audit]
            intent = "Test skill for unit testing purposes"
            rationale = "Needed for tests"

            [skill.audit.provenance]
            type = "expert"
            reference = "Internal testing"

            [skill.population]
            include = ["patient.active == true", "patient.age >= 18"]
            exclude = ["patient.deceased == true"]
        "#;

        let file: SkillPolicyFile = toml::from_str(toml_str).unwrap();
        let pop = file.skills[0].population.as_ref().unwrap();
        assert_eq!(pop.include.len(), 2);
        assert_eq!(pop.exclude.len(), 1);
    }

    // ── Helper ──────────────────────────────────────────────────────

    fn test_skill() -> ClinicalSkillSpec {
        ClinicalSkillSpec {
            id: "ambient_doc.generate_note".into(),
            name: "Ambient Documentation".into(),
            description: None,
            version: "1.0.0".into(),
            severity: Severity::Medium,
            allowed_roles: vec!["physician".into(), "nurse_practitioner".into()],
            action: "ambient_doc.generate_note".into(),
            required_capabilities: vec!["note_generation".into()],
            audit: SkillAudit {
                intent: "Generate structured SOAP notes from encounter transcripts".into(),
                rationale: "Reduces clinician documentation burden".into(),
                provenance: Provenance {
                    type_: ProvenanceType::Sop,
                    reference: "ClinicClaw Clinical Documentation SOP v1.0".into(),
                    uri: Some("https://cliniclaw.internal/sops/ambient-doc-v1".into()),
                    version: Some("1.0".into()),
                },
            },
            population: Some(PopulationGate {
                include: vec!["encounter.status == in-progress".into()],
                exclude: vec!["patient.deceased == true".into()],
            }),
            spec_hash: String::new(),
        }
    }
}
