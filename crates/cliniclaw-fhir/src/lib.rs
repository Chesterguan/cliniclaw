pub mod bundle;
pub mod client;
pub mod error;
pub mod resources;

pub use bundle::{Bundle, BundleEntry};
pub use client::{FhirClient, FhirResource};
pub use error::FhirError;

// Re-export commonly used resource types at crate root
pub use resources::{
    Attachment, CodeableConcept, Coding, DiagnosticReport, DosageInstruction, Encounter,
    EncounterParticipant, HumanName, Identifier, MedicationRequest, Observation, Patient, Period,
    Practitioner, Quantity, Reference, ServiceRequest,
};
