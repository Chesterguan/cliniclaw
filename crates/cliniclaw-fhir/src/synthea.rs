use crate::backend::FhirBackend;
use crate::error::FhirError;
use crate::mock::MockFhirServer;

/// Resource types we import from Synthea bundles.
const SUPPORTED_TYPES: &[&str] = &[
    "Patient",
    "Practitioner",
    "Encounter",
    "Condition",
    "MedicationRequest",
    "Observation",
    "DiagnosticReport",
    "ServiceRequest",
    "DocumentReference",
    "AllergyIntolerance",
    "Procedure",
    "Immunization",
    "CarePlan",
    "Claim",
];

/// Result of importing a Synthea bundle.
#[derive(Debug, Default)]
pub struct SyntheaImportResult {
    pub total_entries: usize,
    pub imported: usize,
    pub skipped: usize,
    pub patients: usize,
    pub encounters: usize,
    pub conditions: usize,
    pub medications: usize,
    pub observations: usize,
}

impl std::fmt::Display for SyntheaImportResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "imported {}/{} resources ({} patients, {} encounters, {} conditions, {} meds, {} obs)",
            self.imported,
            self.total_entries,
            self.patients,
            self.encounters,
            self.conditions,
            self.medications,
            self.observations,
        )
    }
}

/// Load a single Synthea FHIR Bundle JSON file into the mock FHIR server.
///
/// Synthea generates one JSON file per patient, each containing a Bundle
/// with all resources (Patient, Encounters, Conditions, MedicationRequests,
/// Observations, etc.).
///
/// Resources without an `id` field get one assigned from the Bundle entry's
/// `fullUrl` (which Synthea sets to a UUID URN).
pub async fn load_synthea_bundle(
    server: &MockFhirServer,
    bundle_json: &serde_json::Value,
) -> Result<SyntheaImportResult, FhirError> {
    let mut result = SyntheaImportResult::default();

    let entries = bundle_json
        .get("entry")
        .and_then(|e| e.as_array())
        .ok_or_else(|| FhirError::InvalidResource {
            message: "Synthea bundle has no 'entry' array".to_string(),
        })?;

    result.total_entries = entries.len();

    // Build urn:uuid → (ResourceType, id) map once for the whole bundle
    let urn_map = build_urn_map(bundle_json);

    for entry in entries {
        let Some(resource) = entry.get("resource") else {
            result.skipped += 1;
            continue;
        };

        let Some(resource_type) = resource.get("resourceType").and_then(|v| v.as_str()) else {
            result.skipped += 1;
            continue;
        };

        if !SUPPORTED_TYPES.contains(&resource_type) {
            result.skipped += 1;
            continue;
        }

        let mut resource = resource.clone();

        // Synthea often puts the ID in fullUrl as "urn:uuid:<id>" rather than
        // in the resource itself. Extract it if missing.
        if resource.get("id").and_then(|v| v.as_str()).is_none() {
            if let Some(full_url) = entry.get("fullUrl").and_then(|v| v.as_str()) {
                let id = full_url
                    .strip_prefix("urn:uuid:")
                    .unwrap_or(full_url);
                if let Some(obj) = resource.as_object_mut() {
                    obj.insert("id".to_string(), serde_json::Value::String(id.to_string()));
                }
            }
        }

        // Ensure resource has an id before seeding
        if resource.get("id").and_then(|v| v.as_str()).is_none() {
            result.skipped += 1;
            continue;
        }

        // Synthea doesn't always set active:true on Patient resources.
        // Our route handlers default absent active to false (deny by default),
        // so explicitly set it for imported patients.
        if resource_type == "Patient" {
            if let Some(obj) = resource.as_object_mut() {
                obj.entry("active".to_string())
                    .or_insert(serde_json::Value::Bool(true));
            }
        }

        match resource_type {
            "Patient" => result.patients += 1,
            "Encounter" => result.encounters += 1,
            "Condition" => result.conditions += 1,
            "MedicationRequest" => result.medications += 1,
            "Observation" => result.observations += 1,
            _ => {}
        }

        // Normalize urn:uuid: references to FHIR-standard {ResourceType}/{id} format.
        // Synthea uses urn:uuid:xxx for inter-resource references, but route handlers
        // expect Patient/xxx, Encounter/xxx, etc.
        rewrite_refs(&mut resource, &urn_map);

        server.seed(resource).await;
        result.imported += 1;
    }

    Ok(result)
}

/// Build a lookup table from urn:uuid:xxx → (ResourceType, id) for a bundle.
fn build_urn_map(bundle: &serde_json::Value) -> std::collections::HashMap<String, (String, String)> {
    let mut map = std::collections::HashMap::new();
    let entries = bundle.get("entry").and_then(|e| e.as_array());
    for entry in entries.into_iter().flatten() {
        let full_url = entry.get("fullUrl").and_then(|v| v.as_str()).unwrap_or_default();
        let resource = match entry.get("resource") {
            Some(r) => r,
            None => continue,
        };
        let rtype = resource.get("resourceType").and_then(|v| v.as_str()).unwrap_or_default();
        let id = resource
            .get("id")
            .and_then(|v| v.as_str())
            .or_else(|| full_url.strip_prefix("urn:uuid:"))
            .unwrap_or_default();
        if !full_url.is_empty() && !rtype.is_empty() && !id.is_empty() {
            map.insert(full_url.to_string(), (rtype.to_string(), id.to_string()));
        }
    }
    map
}

fn rewrite_refs(value: &mut serde_json::Value, urn_map: &std::collections::HashMap<String, (String, String)>) {
    match value {
        serde_json::Value::Object(map) => {
            // If this object has a "reference" field with a urn:uuid: value, rewrite it
            if let Some(ref_val) = map.get_mut("reference") {
                if let Some(urn) = ref_val.as_str().map(|s| s.to_string()) {
                    if let Some((rtype, id)) = urn_map.get(&urn) {
                        *ref_val = serde_json::Value::String(format!("{}/{}", rtype, id));
                    }
                }
            }
            for v in map.values_mut() {
                rewrite_refs(v, urn_map);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                rewrite_refs(v, urn_map);
            }
        }
        _ => {}
    }
}

/// Maximum number of patient bundles to load from a Synthea directory.
/// Each bundle can be 1-5MB; loading too many wastes memory in demo mode.
const MAX_BUNDLES: usize = 15;

/// Load all Synthea FHIR Bundle JSON files from a directory.
///
/// Expects a directory containing `.json` files, each being a Synthea
/// patient bundle. Loads up to `MAX_BUNDLES` files. Returns the aggregate
/// import result.
pub async fn load_synthea_dir(
    server: &MockFhirServer,
    dir: &std::path::Path,
) -> Result<SyntheaImportResult, FhirError> {
    let mut aggregate = SyntheaImportResult::default();

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| FhirError::InvalidResource {
            message: format!("cannot read Synthea directory {}: {e}", dir.display()),
        })?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |ext| ext == "json")
        })
        .collect();

    // Sort for deterministic ordering
    entries.sort_by_key(|e| e.file_name());

    // Only load first MAX_BUNDLES to keep memory reasonable
    let to_load = entries.len().min(MAX_BUNDLES);
    if entries.len() > MAX_BUNDLES {
        tracing::info!(
            total_files = entries.len(),
            loading = to_load,
            "limiting Synthea bundle load count"
        );
    }

    for entry in &entries[..to_load] {
        let path = entry.path();
        let content = std::fs::read_to_string(&path).map_err(|e| FhirError::InvalidResource {
            message: format!("cannot read {}: {e}", path.display()),
        })?;

        let bundle: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| FhirError::InvalidResource {
                message: format!("invalid JSON in {}: {e}", path.display()),
            })?;

        let result = load_synthea_bundle(server, &bundle).await?;

        tracing::info!(
            file = %path.file_name().unwrap_or_default().to_string_lossy(),
            imported = result.imported,
            skipped = result.skipped,
            "loaded Synthea bundle"
        );

        aggregate.total_entries += result.total_entries;
        aggregate.imported += result.imported;
        aggregate.skipped += result.skipped;
        aggregate.patients += result.patients;
        aggregate.encounters += result.encounters;
        aggregate.conditions += result.conditions;
        aggregate.medications += result.medications;
        aggregate.observations += result.observations;
    }

    Ok(aggregate)
}

/// Activate the most recent encounter per patient for simulation.
///
/// Synthea generates all encounters with `status: "finished"`. This function
/// finds the most recent encounter per living patient and changes its status
/// to `"in-progress"`, making it available for the simulation orchestrator.
///
/// Returns the number of encounters activated.
pub async fn activate_recent_encounters(
    server: &MockFhirServer,
) -> Result<usize, FhirError> {
    // 1. Find all patients (alive only)
    let patient_bundle = server.search_resources("Patient", &[]).await?;
    let patient_entries = patient_bundle
        .get("entry")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    let mut activated = 0;

    for pe in &patient_entries {
        let Some(patient) = pe.get("resource") else { continue };
        let Some(patient_id) = patient.get("id").and_then(|v| v.as_str()) else { continue };

        // Skip deceased patients
        if patient.get("deceasedBoolean").and_then(|v| v.as_bool()).unwrap_or(false) {
            continue;
        }
        if patient.get("deceasedDateTime").is_some() {
            continue;
        }
        // Skip inactive
        if patient.get("active").and_then(|v| v.as_bool()) == Some(false) {
            continue;
        }

        // 2. Find all encounters for this patient
        let enc_bundle = server
            .search_resources("Encounter", &[("patient", patient_id)])
            .await?;
        let enc_entries = enc_bundle
            .get("entry")
            .and_then(|e| e.as_array())
            .cloned()
            .unwrap_or_default();

        // 3. Find the most recent encounter (by period.start, descending)
        let mut best_enc: Option<serde_json::Value> = None;
        let mut best_start = String::new();

        for ee in &enc_entries {
            let Some(enc) = ee.get("resource") else { continue };
            let start = enc
                .pointer("/period/start")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if start >= best_start {
                best_start = start;
                best_enc = Some(enc.clone());
            }
        }

        // 4. Activate it
        if let Some(mut enc) = best_enc {
            if let Some(obj) = enc.as_object_mut() {
                obj.insert(
                    "status".to_string(),
                    serde_json::Value::String("in-progress".to_string()),
                );
            }
            let enc_id = enc.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if !enc_id.is_empty() {
                server.seed(enc).await;
                activated += 1;
            }
        }
    }

    tracing::info!(activated, "activated recent encounters for simulation");
    Ok(activated)
}

/// Filter encounters from the mock FHIR server that are suitable for simulation.
///
/// Returns encounter IDs with `status: "in-progress"` (or optionally other statuses).
/// These can be used to dynamically build simulation pathways.
pub async fn find_active_encounters(
    server: &MockFhirServer,
) -> Result<Vec<ActiveEncounter>, FhirError> {
    let bundle = server
        .search_resources("Encounter", &[("status", "in-progress")])
        .await?;

    let entries = bundle
        .get("entry")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    let mut encounters = Vec::new();
    for entry in &entries {
        let Some(resource) = entry.get("resource") else {
            continue;
        };
        let id = resource
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let patient_ref = resource
            .pointer("/subject/reference")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let patient_display = resource
            .pointer("/subject/display")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let enc_class = resource
            .pointer("/class/code")
            .and_then(|v| v.as_str())
            .unwrap_or("AMB")
            .to_string();
        let reason_codes: Vec<String> = resource
            .get("reasonCode")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|rc| {
                        rc.pointer("/coding/0/code")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();

        if !id.is_empty() && !patient_ref.is_empty() {
            encounters.push(ActiveEncounter {
                encounter_id: id,
                patient_ref,
                patient_display,
                encounter_class: enc_class,
                reason_codes,
            });
        }
    }

    Ok(encounters)
}

/// An encounter ready for simulation with extracted metadata.
#[derive(Debug, Clone)]
pub struct ActiveEncounter {
    pub encounter_id: String,
    pub patient_ref: String,
    pub patient_display: String,
    pub encounter_class: String,
    pub reason_codes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_synthea_bundle() -> serde_json::Value {
        serde_json::json!({
            "resourceType": "Bundle",
            "type": "transaction",
            "entry": [
                {
                    "fullUrl": "urn:uuid:abc-123",
                    "resource": {
                        "resourceType": "Patient",
                        "name": [{"family": "Synthea", "given": ["Test"]}],
                        "gender": "female",
                        "birthDate": "1990-01-01"
                    }
                },
                {
                    "fullUrl": "urn:uuid:enc-456",
                    "resource": {
                        "resourceType": "Encounter",
                        "status": "in-progress",
                        "class": {"code": "AMB"},
                        "subject": {"reference": "Patient/abc-123", "display": "Test Synthea"}
                    }
                },
                {
                    "fullUrl": "urn:uuid:cond-789",
                    "resource": {
                        "resourceType": "Condition",
                        "clinicalStatus": {"coding": [{"code": "active"}]},
                        "code": {"coding": [{"code": "I10", "display": "Hypertension"}]},
                        "subject": {"reference": "Patient/abc-123"}
                    }
                },
                {
                    "fullUrl": "urn:uuid:med-012",
                    "resource": {
                        "resourceType": "MedicationRequest",
                        "status": "active",
                        "intent": "order",
                        "medicationCodeableConcept": {"text": "Lisinopril 10mg"},
                        "subject": {"reference": "Patient/abc-123"}
                    }
                },
                {
                    "fullUrl": "urn:uuid:obs-345",
                    "resource": {
                        "resourceType": "Observation",
                        "status": "final",
                        "code": {"coding": [{"code": "85354-9", "display": "Blood pressure"}]},
                        "subject": {"reference": "Patient/abc-123"}
                    }
                },
                {
                    "resource": {
                        "resourceType": "ExplanationOfBenefit",
                        "id": "eob-skip",
                        "status": "active"
                    }
                }
            ]
        })
    }

    #[tokio::test]
    async fn test_load_synthea_bundle() {
        let server = MockFhirServer::new();
        let bundle = sample_synthea_bundle();
        let result = load_synthea_bundle(&server, &bundle).await.unwrap();

        assert_eq!(result.total_entries, 6);
        assert_eq!(result.imported, 5);
        assert_eq!(result.skipped, 1); // ExplanationOfBenefit
        assert_eq!(result.patients, 1);
        assert_eq!(result.encounters, 1);
        assert_eq!(result.conditions, 1);
        assert_eq!(result.medications, 1);
        assert_eq!(result.observations, 1);

        // Verify ID was extracted from fullUrl
        let patient = server.read_resource("Patient", "abc-123").await.unwrap();
        assert_eq!(patient["name"][0]["family"], "Synthea");
    }

    #[tokio::test]
    async fn test_find_active_encounters() {
        let server = MockFhirServer::new();
        let bundle = sample_synthea_bundle();
        load_synthea_bundle(&server, &bundle).await.unwrap();

        let active = find_active_encounters(&server).await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].encounter_id, "enc-456");
        assert_eq!(active[0].patient_ref, "Patient/abc-123");
    }
}
