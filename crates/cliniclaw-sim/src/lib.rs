//! Long-horizon governance drift engine. See
//! docs/superpowers/specs/2026-06-05-veritas-long-horizon-drift-experiment-design.md

pub mod copyforward;
pub mod epi;
pub mod panel;

#[derive(Debug, thiserror::Error)]
pub enum SimError {
    #[error("epi data error: {0}")]
    Epi(String),
    #[error("panel error: {0}")]
    Panel(String),
    #[error("fhir error: {0}")]
    Fhir(#[from] cliniclaw_fhir::FhirError),
    #[error("persist error: {0}")]
    Persist(#[from] cliniclaw_persist::PersistError),
    #[error("agent error: {0}")]
    Agent(#[from] cliniclaw_agents::AgentError),
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_builds() {
        assert_eq!(2 + 2, 4);
    }
}
