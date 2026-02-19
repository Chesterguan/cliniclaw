pub mod diagnostic_report;
pub mod encounter;
pub mod medication_request;
pub mod observation;
pub mod patient;
pub mod practitioner;
pub mod service_request;
pub mod types;

pub use diagnostic_report::DiagnosticReport;
pub use encounter::{Encounter, EncounterParticipant};
pub use medication_request::MedicationRequest;
pub use observation::Observation;
pub use patient::Patient;
pub use practitioner::Practitioner;
pub use service_request::ServiceRequest;
pub use types::{
    Attachment, CodeableConcept, Coding, DosageInstruction, HumanName, Identifier, Period,
    Quantity, Reference,
};
