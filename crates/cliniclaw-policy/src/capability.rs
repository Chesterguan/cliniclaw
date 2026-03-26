use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::PolicyError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Capability {
    pub id: Uuid,
    pub name: String,
    pub actor_id: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub scope_patient_id: Option<String>,
    pub scope_encounter_id: Option<String>,
}

impl Capability {
    pub fn new(name: impl Into<String>, actor_id: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            actor_id: actor_id.into(),
            issued_at: Utc::now(),
            expires_at: None,
            scope_patient_id: None,
            scope_encounter_id: None,
        }
    }

    pub fn with_expiry(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    pub fn with_patient_scope(mut self, patient_id: impl Into<String>) -> Self {
        self.scope_patient_id = Some(patient_id.into());
        self
    }

    pub fn with_encounter_scope(mut self, encounter_id: impl Into<String>) -> Self {
        self.scope_encounter_id = Some(encounter_id.into());
        self
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at.map_or(false, |exp| Utc::now() > exp)
    }

    pub fn is_valid(&self) -> bool {
        !self.is_expired()
    }

    /// Validate that this capability is valid for the given context.
    /// Checks: not expired, actor matches, and optional scope constraints.
    pub fn validate_for_context(
        &self,
        actor_id: &str,
        patient_id: Option<&str>,
        encounter_id: Option<&str>,
    ) -> Result<(), PolicyError> {
        if self.is_expired() {
            return Err(PolicyError::CapabilityExpired {
                capability: self.name.clone(),
                expired_at: self.expires_at.expect("expires_at must be Some when is_expired() is true"),
            });
        }
        if self.actor_id != actor_id {
            return Err(PolicyError::CapabilityActorMismatch {
                capability: self.name.clone(),
                expected: self.actor_id.clone(),
                actual: actor_id.to_string(),
            });
        }
        if let Some(ref scope_pid) = self.scope_patient_id {
            match patient_id {
                Some(ctx_pid) if scope_pid != ctx_pid => {
                    return Err(PolicyError::CapabilityScopeMismatch {
                        capability: self.name.clone(),
                        scope: format!("patient:{scope_pid}"),
                        actual: format!("patient:{ctx_pid}"),
                    });
                }
                None => {
                    // Capability is patient-scoped but context has no patient_id —
                    // deny rather than silently bypass the scope restriction.
                    return Err(PolicyError::CapabilityScopeMismatch {
                        capability: self.name.clone(),
                        scope: format!("patient:{scope_pid}"),
                        actual: "patient:<missing>".to_string(),
                    });
                }
                _ => {} // scope matches
            }
        }
        if let Some(ref scope_eid) = self.scope_encounter_id {
            match encounter_id {
                Some(ctx_eid) if scope_eid != ctx_eid => {
                    return Err(PolicyError::CapabilityScopeMismatch {
                        capability: self.name.clone(),
                        scope: format!("encounter:{scope_eid}"),
                        actual: format!("encounter:{ctx_eid}"),
                    });
                }
                None => {
                    return Err(PolicyError::CapabilityScopeMismatch {
                        capability: self.name.clone(),
                        scope: format!("encounter:{scope_eid}"),
                        actual: "encounter:<missing>".to_string(),
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn capability_not_expired_by_default() {
        let cap = Capability::new("note_generation", "actor-1");
        assert!(cap.is_valid());
        assert!(!cap.is_expired());
    }

    #[test]
    fn capability_expired() {
        let cap =
            Capability::new("note_generation", "actor-1").with_expiry(Utc::now() - Duration::hours(1));
        assert!(cap.is_expired());
        assert!(!cap.is_valid());
    }

    #[test]
    fn validate_for_context_ok() {
        let cap = Capability::new("note_generation", "actor-1");
        assert!(cap.validate_for_context("actor-1", None, None).is_ok());
    }

    #[test]
    fn validate_for_context_actor_mismatch() {
        let cap = Capability::new("note_generation", "actor-1");
        let result = cap.validate_for_context("actor-2", None, None);
        assert!(matches!(
            result,
            Err(PolicyError::CapabilityActorMismatch { .. })
        ));
    }

    #[test]
    fn validate_for_context_expired() {
        let cap =
            Capability::new("note_generation", "actor-1").with_expiry(Utc::now() - Duration::hours(1));
        let result = cap.validate_for_context("actor-1", None, None);
        assert!(matches!(
            result,
            Err(PolicyError::CapabilityExpired { .. })
        ));
    }

    #[test]
    fn validate_for_context_patient_scope_mismatch() {
        let cap = Capability::new("note_generation", "actor-1").with_patient_scope("patient-A");
        let result = cap.validate_for_context("actor-1", Some("patient-B"), None);
        assert!(matches!(
            result,
            Err(PolicyError::CapabilityScopeMismatch { .. })
        ));
    }

    #[test]
    fn validate_for_context_encounter_scope_mismatch() {
        let cap = Capability::new("note_generation", "actor-1").with_encounter_scope("enc-1");
        let result = cap.validate_for_context("actor-1", None, Some("enc-2"));
        assert!(matches!(
            result,
            Err(PolicyError::CapabilityScopeMismatch { .. })
        ));
    }

    #[test]
    fn validate_for_context_scope_match() {
        let cap = Capability::new("note_generation", "actor-1")
            .with_patient_scope("patient-A")
            .with_encounter_scope("enc-1");
        assert!(cap
            .validate_for_context("actor-1", Some("patient-A"), Some("enc-1"))
            .is_ok());
    }

    #[test]
    fn validate_for_context_no_scope_always_passes() {
        let cap = Capability::new("note_generation", "actor-1");
        assert!(cap
            .validate_for_context("actor-1", Some("any-patient"), Some("any-enc"))
            .is_ok());
    }

    #[test]
    fn validate_scope_denied_when_context_missing() {
        // Capability scoped to patient-A, but context has no patient_id — must deny
        // to prevent scope bypass attacks.
        let cap = Capability::new("note_generation", "actor-1").with_patient_scope("patient-A");
        let result = cap.validate_for_context("actor-1", None, None);
        assert!(matches!(
            result,
            Err(PolicyError::CapabilityScopeMismatch { .. })
        ));
    }

    #[test]
    fn validate_encounter_scope_denied_when_context_missing() {
        let cap = Capability::new("note_generation", "actor-1").with_encounter_scope("enc-1");
        let result = cap.validate_for_context("actor-1", None, None);
        assert!(matches!(
            result,
            Err(PolicyError::CapabilityScopeMismatch { .. })
        ));
    }
}
