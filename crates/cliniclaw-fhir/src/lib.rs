pub mod backend;
pub mod bundle;
pub mod client;
pub mod error;
pub mod mock;
pub mod mock_data;
pub mod resources;

pub use backend::FhirBackend;
pub use bundle::{Bundle, BundleEntry};
pub use client::{FhirClient, FhirResource};
pub use error::FhirError;
pub use mock::MockFhirServer;

// Re-export commonly used resource types at crate root
pub use resources::{
    Attachment, CodeableConcept, Coding, DiagnosticReport, DoseAndRate, DosageInstruction,
    Encounter, EncounterParticipant, HumanName, Identifier, MedicationRequest, Observation,
    Patient, Period, Practitioner, Quantity, Reference, ServiceRequest,
};
