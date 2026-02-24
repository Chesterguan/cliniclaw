use std::sync::Arc;

use axum::{
    extract::{Json, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;

use crate::error::ApiError;
use crate::state::AppState;
use super::{extract_bearer_token, is_valid_fhir_id};

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct WorklistQuery {
    pub practitioner_id: String,
}

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct WorklistResponse {
    pub practitioner_id: String,
    pub entries: Vec<WorklistEntry>,
    pub total: u32,
}

#[derive(Debug, serde::Serialize)]
pub struct WorklistEntry {
    pub encounter: WorklistEncounter,
    pub patient: WorklistPatient,
    pub allergies: Vec<String>,
    pub problem_list: Vec<WorklistCondition>,
    pub active_medications_count: u32,
    pub pending_orders_count: u32,
    pub flags: SafetyFlags,
}

#[derive(Debug, serde::Serialize)]
pub struct WorklistEncounter {
    pub id: String,
    pub status: String,
    /// FHIR class code (e.g. "AMB", "IMP"). Empty string when absent so the
    /// frontend can call .toUpperCase() without a null check.
    pub class_code: String,
    /// Renamed from period_start to match the frontend type (WorklistEncounter.start_time).
    pub start_time: Option<String>,
    /// From encounter.location[0].location.display, if present.
    pub location: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct WorklistPatient {
    pub id: String,
    /// Display name formatted as "Family, Given" or fallback to id.
    pub name: String,
    /// Patient birth date (YYYY-MM-DD) from birthDate field.
    pub birth_date: Option<String>,
    /// Patient gender string from gender field.
    pub gender: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct WorklistCondition {
    /// ICD-10 or other coding code — empty string if absent.
    pub code: String,
    /// Human-readable display text — empty string if absent.
    pub display: String,
}

#[derive(Debug, serde::Serialize)]
pub struct SafetyFlags {
    /// Patient is marked as deceased
    pub deceased: bool,
    /// Patient record is inactive
    pub inactive: bool,
}

// ── Handler ───────────────────────────────────────────────────────────────────

pub async fn get_worklist(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<WorklistQuery>,
) -> Result<Json<WorklistResponse>, ApiError> {
    // 0a. Authenticate
    let _bearer = extract_bearer_token(&headers)?;
    // TODO: validate token against SMART-on-FHIR / API key store

    // 0b. Validate practitioner ID
    if !is_valid_fhir_id(&query.practitioner_id) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "invalid practitioner_id format"));
    }

    tracing::info!(
        practitioner_id = %query.practitioner_id,
        "fetching worklist"
    );

    // 1. Search all in-progress encounters for this practitioner.
    //    The mock backend matches top-level fields; the live backend uses FHIR search params.
    //    We search by status=in-progress and filter by participant on the client side
    //    because the FHIR search parameter "participant" is implementation-dependent.
    let participant_ref = format!("Practitioner/{}", query.practitioner_id);

    // Search encounters as raw JSON so we can extract both typed fields (via
    // serde) and untyped fields like location without a second FHIR fetch.
    let enc_bundle_raw = state
        .fhir
        .search_resources("Encounter", &[("status", "in-progress")])
        .await
        .map_err(ApiError::from)?;

    let enc_entries: Vec<serde_json::Value> = enc_bundle_raw
        .get("entry")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    // Deserialize + filter by participant
    let encounters: Vec<(cliniclaw_fhir::Encounter, serde_json::Value)> = enc_entries
        .into_iter()
        .filter_map(|entry| {
            let resource = entry.get("resource")?.clone();
            let enc: cliniclaw_fhir::Encounter = serde_json::from_value(resource.clone()).ok()?;
            // Filter by participant
            let is_participant = enc
                .participant
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .any(|p| {
                    p.individual
                        .as_ref()
                        .and_then(|r| r.reference.as_deref())
                        .map_or(false, |r| r == participant_ref || r.ends_with(&query.practitioner_id))
                });
            if is_participant { Some((enc, resource)) } else { None }
        })
        .collect();

    // 2. For each encounter, fetch the patient + conditions + med count.
    let mut entries = Vec::with_capacity(encounters.len());

    for (encounter, encounter_raw) in encounters {
        let enc_id = encounter.id.as_deref().unwrap_or("unknown").to_string();

        // Extract patient ID from subject reference
        let patient_id = match encounter
            .subject
            .as_ref()
            .and_then(|s| s.reference.as_deref())
            .and_then(|r| r.strip_prefix("Patient/"))
        {
            Some(id) => id.to_string(),
            None => {
                tracing::warn!(encounter_id = %enc_id, "skipping encounter with no Patient/ subject");
                continue;
            }
        };

        // Fetch patient as raw JSON so we can extract both the typed fields
        // (name, birthDate, gender, active, deceased) and untyped extensions (allergies).
        let patient_raw = match state
            .fhir
            .read_resource("Patient", &patient_id)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(patient_id = %patient_id, error = %e, "skipping patient unavailable in FHIR");
                continue;
            }
        };

        // Deserialize into the typed Patient struct for structured fields.
        let patient: cliniclaw_fhir::Patient =
            match serde_json::from_value(patient_raw.clone()) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(patient_id = %patient_id, error = %e, "skipping patient with invalid structure");
                    continue;
                }
            };

        // Build display name: "Family, Given" using the first HumanName entry.
        let display_name = patient
            .name
            .as_deref()
            .and_then(|names| names.first())
            .map(|hn| {
                let family = hn.family.as_deref().unwrap_or("");
                let given = hn
                    .given
                    .as_deref()
                    .and_then(|g| g.first())
                    .map(String::as_str)
                    .unwrap_or("");
                match (family, given) {
                    ("", "") => patient_id.clone(),
                    (f, "") => f.to_string(),
                    ("", g) => g.to_string(),
                    (f, g) => format!("{}, {}", f, g),
                }
            })
            .unwrap_or_else(|| patient_id.clone());

        // Extract allergies from Patient extensions with the allergy-summary URL.
        // Extensions are not part of the typed struct — read directly from raw JSON.
        let allergies: Vec<String> = patient_raw
            .get("extension")
            .and_then(|e| e.as_array())
            .map(|exts| {
                exts.iter()
                    .filter(|ext| {
                        ext.get("url")
                            .and_then(|u| u.as_str())
                            .map_or(false, |u| u == "http://cliniclaw.dev/fhir/allergy-summary")
                    })
                    .filter_map(|ext| {
                        ext.get("valueString")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Extract encounter location from the raw JSON we already have —
        // no need for a second FHIR fetch.
        let enc_location: Option<String> = encounter_raw
            .pointer("/location/0/location/display")
            .and_then(|d| d.as_str())
            .map(String::from);

        // Fetch active MedicationRequests to get count
        let med_bundle = state
            .fhir
            .search_resources(
                "MedicationRequest",
                &[
                    ("subject", &format!("Patient/{}", patient_id)),
                    ("status", "active"),
                ],
            )
            .await
            .unwrap_or_else(|_| serde_json::json!({"total": 0, "entry": []}));

        let active_medications_count = med_bundle
            .get("total")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        // Fetch conditions via raw FHIR search (Condition type not in typed search helpers)
        let cond_bundle = state
            .fhir
            .search_resources(
                "Condition",
                &[("subject", &format!("Patient/{}", patient_id))],
            )
            .await
            .unwrap_or_else(|_| serde_json::json!({"entry": []}));

        let problem_list: Vec<WorklistCondition> = cond_bundle
            .get("entry")
            .and_then(|e| e.as_array())
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| {
                        let resource = entry.get("resource")?;
                        let code_text = resource
                            .pointer("/code/text")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let coding_display = resource
                            .pointer("/code/coding/0/display")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let coding_code = resource
                            .pointer("/code/coding/0/code")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        Some(WorklistCondition {
                            // Use empty string fallback so the frontend type (non-optional) works.
                            code: coding_code.unwrap_or_default(),
                            display: code_text.or(coding_display).unwrap_or_default(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let patient_active = patient.active.unwrap_or(true);
        let patient_deceased = patient.is_deceased().unwrap_or(false);

        entries.push(WorklistEntry {
            encounter: WorklistEncounter {
                id: enc_id,
                status: encounter.status.clone(),
                // Unwrap to empty string so the frontend can call .toUpperCase() unconditionally.
                class_code: encounter.class_.as_ref().and_then(|c| c.code.clone()).unwrap_or_default(),
                start_time: encounter.period.as_ref().and_then(|p| p.start.clone()),
                location: enc_location,
            },
            patient: WorklistPatient {
                id: patient_id,
                name: display_name,
                birth_date: patient.birth_date.clone(),
                gender: patient.gender.clone(),
            },
            allergies,
            problem_list,
            active_medications_count,
            pending_orders_count: 0,
            flags: SafetyFlags {
                deceased: patient_deceased,
                inactive: !patient_active,
            },
        });
    }

    let total = entries.len() as u32;

    tracing::info!(
        practitioner_id = %query.practitioner_id,
        entries = total,
        "worklist fetched"
    );

    Ok(Json(WorklistResponse {
        practitioner_id: query.practitioner_id,
        entries,
        total,
    }))
}
