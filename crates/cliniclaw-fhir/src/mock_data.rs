/// Clinically realistic seed data for mock/demo mode.
///
/// 8 synthetic patients with allergies, medications, problems,
/// encounters, and edge cases. All data is fictitious.

/// Generate all seed resources for mock mode.
pub fn seed_resources() -> Vec<serde_json::Value> {
    let mut resources = Vec::new();

    // Practitioner
    resources.push(practitioner_wilson());

    // Patients
    resources.extend(patients());

    // Encounters (one per active patient)
    resources.extend(encounters());

    // Conditions (problem lists)
    resources.extend(conditions());

    // MedicationRequests (active meds)
    resources.extend(medication_requests());

    // ServiceRequests (for prior auth demo)
    resources.push(service_request_tkr());

    resources
}

fn practitioner_wilson() -> serde_json::Value {
    serde_json::json!({
        "resourceType": "Practitioner",
        "id": "practitioner-001",
        "name": [{"family": "Wilson", "given": ["James"], "prefix": ["Dr."]}],
        "identifier": [{"system": "http://hl7.org/fhir/sid/us-npi", "value": "1234567890"}],
        "qualification": [{"code": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/v2-0360", "code": "MD"}]}}]
    })
}

fn patients() -> Vec<serde_json::Value> {
    vec![
        // patient-001: Sarah Mitchell, 40F, routine HTN visit
        serde_json::json!({
            "resourceType": "Patient",
            "id": "patient-001",
            "active": true,
            "name": [{"family": "Mitchell", "given": ["Sarah"]}],
            "gender": "female",
            "birthDate": "1985-03-15",
            "identifier": [{"system": "http://hospital.example/mrn", "value": "MRN-001"}],
            "extension": [{
                "url": "http://cliniclaw.dev/fhir/allergy-summary",
                "valueString": "Penicillin"
            }]
        }),
        // patient-002: James Thompson, 53M, T2DM management
        serde_json::json!({
            "resourceType": "Patient",
            "id": "patient-002",
            "active": true,
            "name": [{"family": "Thompson", "given": ["James"]}],
            "gender": "male",
            "birthDate": "1972-08-22",
            "identifier": [{"system": "http://hospital.example/mrn", "value": "MRN-002"}]
        }),
        // patient-003: Maria Garcia, 35F, healthy prenatal
        serde_json::json!({
            "resourceType": "Patient",
            "id": "patient-003",
            "active": true,
            "name": [{"family": "Garcia", "given": ["Maria"]}],
            "gender": "female",
            "birthDate": "1990-11-07",
            "identifier": [{"system": "http://hospital.example/mrn", "value": "MRN-003"}]
        }),
        // patient-004: Robert Chen, 70M, COPD exacerbation
        serde_json::json!({
            "resourceType": "Patient",
            "id": "patient-004",
            "active": true,
            "name": [{"family": "Chen", "given": ["Robert"]}],
            "gender": "male",
            "birthDate": "1955-06-30",
            "identifier": [{"system": "http://hospital.example/mrn", "value": "MRN-004"}]
        }),
        // patient-005: Emily Johnson, 57F, knee OA, needs TKR prior auth
        serde_json::json!({
            "resourceType": "Patient",
            "id": "patient-005",
            "active": true,
            "name": [{"family": "Johnson", "given": ["Emily"]}],
            "gender": "female",
            "birthDate": "1968-12-03",
            "identifier": [{"system": "http://hospital.example/mrn", "value": "MRN-005"}]
        }),
        // patient-006: David Williams, 82M, CHF + high-risk meds
        serde_json::json!({
            "resourceType": "Patient",
            "id": "patient-006",
            "active": true,
            "name": [{"family": "Williams", "given": ["David"]}],
            "gender": "male",
            "birthDate": "1943-04-18",
            "identifier": [{"system": "http://hospital.example/mrn", "value": "MRN-006"}]
        }),
        // patient-007: Deceased patient (edge case)
        serde_json::json!({
            "resourceType": "Patient",
            "id": "patient-007",
            "active": true,
            "name": [{"family": "Doe", "given": ["John"]}],
            "gender": "male",
            "birthDate": "1950-01-01",
            "deceasedBoolean": true,
            "identifier": [{"system": "http://hospital.example/mrn", "value": "MRN-007"}]
        }),
        // patient-008: Inactive patient (edge case)
        serde_json::json!({
            "resourceType": "Patient",
            "id": "patient-008",
            "active": false,
            "name": [{"family": "Inactive", "given": ["Record"]}],
            "gender": "other",
            "birthDate": "1980-05-20",
            "identifier": [{"system": "http://hospital.example/mrn", "value": "MRN-008"}]
        }),
    ]
}

fn encounters() -> Vec<serde_json::Value> {
    vec![
        // enc-001: Sarah Mitchell, routine ambulatory
        serde_json::json!({
            "resourceType": "Encounter",
            "id": "enc-001",
            "status": "in-progress",
            "class": {"code": "AMB", "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode", "display": "ambulatory"},
            "subject": {"reference": "Patient/patient-001", "display": "Sarah Mitchell"},
            "participant": [{"individual": {"reference": "Practitioner/practitioner-001", "display": "Dr. James Wilson"}}],
            "period": {"start": "2026-02-19T09:00:00Z"},
            "reasonCode": [{"coding": [{"system": "http://snomed.info/sct", "code": "38341003", "display": "Hypertension"}]}]
        }),
        // enc-002: James Thompson, T2DM follow-up
        serde_json::json!({
            "resourceType": "Encounter",
            "id": "enc-002",
            "status": "in-progress",
            "class": {"code": "AMB", "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode", "display": "ambulatory"},
            "subject": {"reference": "Patient/patient-002", "display": "James Thompson"},
            "participant": [{"individual": {"reference": "Practitioner/practitioner-001", "display": "Dr. James Wilson"}}],
            "period": {"start": "2026-02-19T10:30:00Z"}
        }),
        // enc-003: Maria Garcia, prenatal
        serde_json::json!({
            "resourceType": "Encounter",
            "id": "enc-003",
            "status": "in-progress",
            "class": {"code": "AMB", "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode", "display": "ambulatory"},
            "subject": {"reference": "Patient/patient-003", "display": "Maria Garcia"},
            "participant": [{"individual": {"reference": "Practitioner/practitioner-001", "display": "Dr. James Wilson"}}],
            "period": {"start": "2026-02-19T11:00:00Z"}
        }),
        // enc-004: Robert Chen, COPD exacerbation (inpatient)
        serde_json::json!({
            "resourceType": "Encounter",
            "id": "enc-004",
            "status": "in-progress",
            "class": {"code": "IMP", "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode", "display": "inpatient"},
            "subject": {"reference": "Patient/patient-004", "display": "Robert Chen"},
            "participant": [{"individual": {"reference": "Practitioner/practitioner-001", "display": "Dr. James Wilson"}}],
            "period": {"start": "2026-02-18T06:00:00Z"},
            "location": [{"location": {"display": "Room 308"}}]
        }),
        // enc-005: Emily Johnson, OA consultation
        serde_json::json!({
            "resourceType": "Encounter",
            "id": "enc-005",
            "status": "in-progress",
            "class": {"code": "AMB", "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode", "display": "ambulatory"},
            "subject": {"reference": "Patient/patient-005", "display": "Emily Johnson"},
            "participant": [{"individual": {"reference": "Practitioner/practitioner-001", "display": "Dr. James Wilson"}}],
            "period": {"start": "2026-02-19T13:00:00Z"}
        }),
        // enc-006: David Williams, CHF management
        serde_json::json!({
            "resourceType": "Encounter",
            "id": "enc-006",
            "status": "in-progress",
            "class": {"code": "AMB", "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode", "display": "ambulatory"},
            "subject": {"reference": "Patient/patient-006", "display": "David Williams"},
            "participant": [{"individual": {"reference": "Practitioner/practitioner-001", "display": "Dr. James Wilson"}}],
            "period": {"start": "2026-02-19T14:00:00Z"},
            "location": [{"location": {"display": "Room 412"}}]
        }),
    ]
}

fn conditions() -> Vec<serde_json::Value> {
    vec![
        // Sarah Mitchell: HTN
        serde_json::json!({
            "resourceType": "Condition",
            "id": "cond-001",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": "active"}]},
            "code": {"coding": [{"system": "http://hl7.org/fhir/sid/icd-10-cm", "code": "I10", "display": "Essential hypertension"}], "text": "Essential hypertension"},
            "subject": {"reference": "Patient/patient-001"}
        }),
        // James Thompson: T2DM + obesity
        serde_json::json!({
            "resourceType": "Condition",
            "id": "cond-002",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": "active"}]},
            "code": {"coding": [{"system": "http://hl7.org/fhir/sid/icd-10-cm", "code": "E11.9", "display": "Type 2 diabetes mellitus without complications"}], "text": "Type 2 diabetes mellitus"},
            "subject": {"reference": "Patient/patient-002"}
        }),
        serde_json::json!({
            "resourceType": "Condition",
            "id": "cond-003",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": "active"}]},
            "code": {"coding": [{"system": "http://hl7.org/fhir/sid/icd-10-cm", "code": "E66.01", "display": "Morbid obesity due to excess calories"}], "text": "Obesity"},
            "subject": {"reference": "Patient/patient-002"}
        }),
        // Maria Garcia: pregnancy
        serde_json::json!({
            "resourceType": "Condition",
            "id": "cond-004",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": "active"}]},
            "code": {"coding": [{"system": "http://hl7.org/fhir/sid/icd-10-cm", "code": "Z33.1", "display": "Pregnant state, incidental"}], "text": "Pregnancy"},
            "subject": {"reference": "Patient/patient-003"}
        }),
        // Robert Chen: COPD + CAD
        serde_json::json!({
            "resourceType": "Condition",
            "id": "cond-005",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": "active"}]},
            "code": {"coding": [{"system": "http://hl7.org/fhir/sid/icd-10-cm", "code": "J44.1", "display": "COPD with acute exacerbation"}], "text": "COPD with acute exacerbation"},
            "subject": {"reference": "Patient/patient-004"}
        }),
        serde_json::json!({
            "resourceType": "Condition",
            "id": "cond-006",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": "active"}]},
            "code": {"coding": [{"system": "http://hl7.org/fhir/sid/icd-10-cm", "code": "I25.10", "display": "Atherosclerotic heart disease of native coronary artery"}], "text": "Coronary artery disease"},
            "subject": {"reference": "Patient/patient-004"}
        }),
        // Emily Johnson: bilateral knee OA
        serde_json::json!({
            "resourceType": "Condition",
            "id": "cond-007",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": "active"}]},
            "code": {"coding": [{"system": "http://hl7.org/fhir/sid/icd-10-cm", "code": "M17.0", "display": "Bilateral primary osteoarthritis of knee"}], "text": "Bilateral knee osteoarthritis"},
            "subject": {"reference": "Patient/patient-005"}
        }),
        // David Williams: CHF + AFib
        serde_json::json!({
            "resourceType": "Condition",
            "id": "cond-008",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": "active"}]},
            "code": {"coding": [{"system": "http://hl7.org/fhir/sid/icd-10-cm", "code": "I50.9", "display": "Heart failure, unspecified"}], "text": "Congestive heart failure"},
            "subject": {"reference": "Patient/patient-006"}
        }),
        serde_json::json!({
            "resourceType": "Condition",
            "id": "cond-009",
            "clinicalStatus": {"coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": "active"}]},
            "code": {"coding": [{"system": "http://hl7.org/fhir/sid/icd-10-cm", "code": "I48.91", "display": "Unspecified atrial fibrillation"}], "text": "Atrial fibrillation"},
            "subject": {"reference": "Patient/patient-006"}
        }),
    ]
}

fn medication_requests() -> Vec<serde_json::Value> {
    vec![
        // Sarah Mitchell: lisinopril
        serde_json::json!({
            "resourceType": "MedicationRequest",
            "id": "medrq-001",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {"coding": [{"system": "http://www.nlm.nih.gov/research/umls/rxnorm", "code": "314076", "display": "Lisinopril 10 MG Oral Tablet"}], "text": "Lisinopril 10mg"},
            "subject": {"reference": "Patient/patient-001"},
            "dosageInstruction": [{"text": "10mg PO daily", "route": {"text": "oral"}, "doseAndRate": [{"doseQuantity": {"value": 10, "unit": "mg"}}]}]
        }),
        // James Thompson: metformin + glipizide
        serde_json::json!({
            "resourceType": "MedicationRequest",
            "id": "medrq-002",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {"coding": [{"system": "http://www.nlm.nih.gov/research/umls/rxnorm", "code": "860975", "display": "Metformin 1000 MG Oral Tablet"}], "text": "Metformin 1000mg"},
            "subject": {"reference": "Patient/patient-002"},
            "dosageInstruction": [{"text": "1000mg PO BID with meals", "route": {"text": "oral"}}]
        }),
        serde_json::json!({
            "resourceType": "MedicationRequest",
            "id": "medrq-003",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {"coding": [{"system": "http://www.nlm.nih.gov/research/umls/rxnorm", "code": "310488", "display": "Glipizide 5 MG Oral Tablet"}], "text": "Glipizide 5mg"},
            "subject": {"reference": "Patient/patient-002"},
            "dosageInstruction": [{"text": "5mg PO daily before breakfast", "route": {"text": "oral"}}]
        }),
        // Maria Garcia: prenatal vitamin
        serde_json::json!({
            "resourceType": "MedicationRequest",
            "id": "medrq-004",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {"text": "Prenatal vitamin"},
            "subject": {"reference": "Patient/patient-003"},
            "dosageInstruction": [{"text": "1 tablet PO daily", "route": {"text": "oral"}}]
        }),
        // Robert Chen: tiotropium, albuterol PRN, prednisone taper, fluticasone
        serde_json::json!({
            "resourceType": "MedicationRequest",
            "id": "medrq-005",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {"coding": [{"system": "http://www.nlm.nih.gov/research/umls/rxnorm", "code": "1552104", "display": "Tiotropium 18 MCG Inhalation Powder"}], "text": "Tiotropium 18mcg"},
            "subject": {"reference": "Patient/patient-004"},
            "dosageInstruction": [{"text": "18mcg inhaled daily", "route": {"text": "inhalation"}}]
        }),
        serde_json::json!({
            "resourceType": "MedicationRequest",
            "id": "medrq-006",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {"coding": [{"system": "http://www.nlm.nih.gov/research/umls/rxnorm", "code": "245314", "display": "Albuterol 90 MCG/ACTUAT Metered Dose Inhaler"}], "text": "Albuterol MDI"},
            "subject": {"reference": "Patient/patient-004"},
            "dosageInstruction": [{"text": "2 puffs inhaled Q4-6H PRN", "route": {"text": "inhalation"}, "asNeededBoolean": true}]
        }),
        serde_json::json!({
            "resourceType": "MedicationRequest",
            "id": "medrq-007",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {"coding": [{"system": "http://www.nlm.nih.gov/research/umls/rxnorm", "code": "312617", "display": "Prednisone 20 MG Oral Tablet"}], "text": "Prednisone taper"},
            "subject": {"reference": "Patient/patient-004"},
            "dosageInstruction": [{"text": "40mg PO daily x3d, then 20mg x3d, then 10mg x3d", "route": {"text": "oral"}}]
        }),
        serde_json::json!({
            "resourceType": "MedicationRequest",
            "id": "medrq-008",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {"coding": [{"system": "http://www.nlm.nih.gov/research/umls/rxnorm", "code": "896188", "display": "Fluticasone Propionate 250 MCG/ACTUAT"}], "text": "Fluticasone 250mcg"},
            "subject": {"reference": "Patient/patient-004"},
            "dosageInstruction": [{"text": "250mcg inhaled BID", "route": {"text": "inhalation"}}]
        }),
        // Emily Johnson: naproxen + acetaminophen PRN
        serde_json::json!({
            "resourceType": "MedicationRequest",
            "id": "medrq-009",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {"coding": [{"system": "http://www.nlm.nih.gov/research/umls/rxnorm", "code": "849727", "display": "Naproxen 500 MG Oral Tablet"}], "text": "Naproxen 500mg"},
            "subject": {"reference": "Patient/patient-005"},
            "dosageInstruction": [{"text": "500mg PO BID with food", "route": {"text": "oral"}}]
        }),
        serde_json::json!({
            "resourceType": "MedicationRequest",
            "id": "medrq-010",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {"coding": [{"system": "http://www.nlm.nih.gov/research/umls/rxnorm", "code": "313782", "display": "Acetaminophen 500 MG Oral Tablet"}], "text": "Acetaminophen 500mg"},
            "subject": {"reference": "Patient/patient-005"},
            "dosageInstruction": [{"text": "500mg PO Q6H PRN pain", "route": {"text": "oral"}, "asNeededBoolean": true}]
        }),
        // David Williams: carvedilol, furosemide, warfarin (HIGH RISK)
        serde_json::json!({
            "resourceType": "MedicationRequest",
            "id": "medrq-011",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {"coding": [{"system": "http://www.nlm.nih.gov/research/umls/rxnorm", "code": "200031", "display": "Carvedilol 25 MG Oral Tablet"}], "text": "Carvedilol 25mg"},
            "subject": {"reference": "Patient/patient-006"},
            "dosageInstruction": [{"text": "25mg PO BID", "route": {"text": "oral"}}]
        }),
        serde_json::json!({
            "resourceType": "MedicationRequest",
            "id": "medrq-012",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {"coding": [{"system": "http://www.nlm.nih.gov/research/umls/rxnorm", "code": "310429", "display": "Furosemide 40 MG Oral Tablet"}], "text": "Furosemide 40mg"},
            "subject": {"reference": "Patient/patient-006"},
            "dosageInstruction": [{"text": "40mg PO daily", "route": {"text": "oral"}}]
        }),
        serde_json::json!({
            "resourceType": "MedicationRequest",
            "id": "medrq-013",
            "status": "active",
            "intent": "order",
            "medicationCodeableConcept": {"coding": [{"system": "http://www.nlm.nih.gov/research/umls/rxnorm", "code": "855288", "display": "Warfarin Sodium 5 MG Oral Tablet"}], "text": "Warfarin 5mg"},
            "subject": {"reference": "Patient/patient-006"},
            "dosageInstruction": [{"text": "5mg PO daily", "route": {"text": "oral"}}]
        }),
    ]
}

fn service_request_tkr() -> serde_json::Value {
    serde_json::json!({
        "resourceType": "ServiceRequest",
        "id": "srvreq-001",
        "status": "active",
        "intent": "order",
        "code": {"coding": [{"system": "http://www.ama-assn.org/go/cpt", "code": "27447", "display": "Total knee arthroplasty"}], "text": "Total Knee Replacement"},
        "subject": {"reference": "Patient/patient-005", "display": "Emily Johnson"},
        "encounter": {"reference": "Encounter/enc-005"},
        "requester": {"reference": "Practitioner/practitioner-001", "display": "Dr. James Wilson"},
        "reasonCode": [{"coding": [{"system": "http://hl7.org/fhir/sid/icd-10-cm", "code": "M17.0", "display": "Bilateral primary osteoarthritis of knee"}]}],
        "note": [{"text": "Patient has failed 6 months of conservative management including PT, NSAIDs, and corticosteroid injections."}]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seed_resources_count() {
        let resources = seed_resources();
        // 1 practitioner + 8 patients + 6 encounters + 9 conditions + 13 med requests + 1 service request = 38
        assert_eq!(resources.len(), 38);
    }

    #[test]
    fn test_all_resources_have_id_and_type() {
        for resource in seed_resources() {
            assert!(
                resource.get("resourceType").is_some(),
                "missing resourceType: {:?}",
                resource.get("id")
            );
            assert!(
                resource.get("id").is_some(),
                "missing id for {}",
                resource["resourceType"]
            );
        }
    }

    #[test]
    fn test_edge_case_patients() {
        let resources = seed_resources();
        let deceased = resources
            .iter()
            .find(|r| r.get("id").and_then(|v| v.as_str()) == Some("patient-007"))
            .unwrap();
        assert_eq!(deceased["deceasedBoolean"], true);

        let inactive = resources
            .iter()
            .find(|r| r.get("id").and_then(|v| v.as_str()) == Some("patient-008"))
            .unwrap();
        assert_eq!(inactive["active"], false);
    }
}
