use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::backend::FhirBackend;
use crate::error::FhirError;

/// In-memory FHIR backend for demo/test mode.
///
/// Stores resources as `serde_json::Value` keyed by `"{ResourceType}/{id}"`.
/// Thread-safe via `Arc<RwLock<...>>`.
#[derive(Debug, Clone)]
pub struct MockFhirServer {
    store: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

impl MockFhirServer {
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Seed a resource into the store.
    pub async fn seed(&self, resource: serde_json::Value) {
        let resource_type = resource
            .get("resourceType")
            .and_then(|v| v.as_str())
            .expect("seeded resource must have resourceType");
        let id = resource
            .get("id")
            .and_then(|v| v.as_str())
            .expect("seeded resource must have id");
        let key = format!("{}/{}", resource_type, id);
        self.store.write().await.insert(key, resource);
    }

    /// Seed multiple resources at once.
    pub async fn seed_all(&self, resources: Vec<serde_json::Value>) {
        let mut store = self.store.write().await;
        for resource in resources {
            let resource_type = resource
                .get("resourceType")
                .and_then(|v| v.as_str())
                .expect("seeded resource must have resourceType");
            let id = resource
                .get("id")
                .and_then(|v| v.as_str())
                .expect("seeded resource must have id");
            let key = format!("{}/{}", resource_type, id);
            store.insert(key, resource);
        }
    }

    /// Return the number of resources in the store.
    pub async fn count(&self) -> usize {
        self.store.read().await.len()
    }
}

impl Default for MockFhirServer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FhirBackend for MockFhirServer {
    async fn read_resource(
        &self,
        resource_type: &str,
        id: &str,
    ) -> Result<serde_json::Value, FhirError> {
        let key = format!("{}/{}", resource_type, id);
        let store = self.store.read().await;
        store.get(&key).cloned().ok_or_else(|| FhirError::NotFound {
            resource_type: resource_type.to_string(),
            id: id.to_string(),
        })
    }

    async fn create_resource(
        &self,
        resource_type: &str,
        resource: &serde_json::Value,
    ) -> Result<serde_json::Value, FhirError> {
        let mut val = resource.clone();

        // Assign an ID if not present
        let id = if let Some(existing_id) = val.get("id").and_then(|v| v.as_str()) {
            existing_id.to_string()
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            val.as_object_mut()
                .ok_or_else(|| FhirError::InvalidResource {
                    message: "resource must be a JSON object".to_string(),
                })?
                .insert("id".to_string(), serde_json::Value::String(id.clone()));
            id
        };

        // Ensure resourceType is set
        val.as_object_mut()
            .unwrap()
            .entry("resourceType".to_string())
            .or_insert_with(|| serde_json::Value::String(resource_type.to_string()));

        let key = format!("{}/{}", resource_type, id);
        self.store.write().await.insert(key, val.clone());
        Ok(val)
    }

    async fn update_resource(
        &self,
        resource_type: &str,
        id: &str,
        resource: &serde_json::Value,
    ) -> Result<serde_json::Value, FhirError> {
        let key = format!("{}/{}", resource_type, id);
        let mut store = self.store.write().await;

        if !store.contains_key(&key) {
            return Err(FhirError::NotFound {
                resource_type: resource_type.to_string(),
                id: id.to_string(),
            });
        }

        let mut val = resource.clone();
        val.as_object_mut()
            .ok_or_else(|| FhirError::InvalidResource {
                message: "resource must be a JSON object".to_string(),
            })?
            .insert("id".to_string(), serde_json::Value::String(id.to_string()));

        store.insert(key, val.clone());
        Ok(val)
    }

    async fn search_resources(
        &self,
        resource_type: &str,
        params: &[(&str, &str)],
    ) -> Result<serde_json::Value, FhirError> {
        let store = self.store.read().await;
        let prefix = format!("{}/", resource_type);

        let mut entries: Vec<serde_json::Value> = store
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .filter(|(_, v)| matches_params(v, params))
            .map(|(_, v)| {
                let full_url = format!(
                    "{}/{}",
                    resource_type,
                    v.get("id").and_then(|i| i.as_str()).unwrap_or("unknown")
                );
                serde_json::json!({
                    "fullUrl": full_url,
                    "resource": v
                })
            })
            .collect();

        entries.sort_by(|a, b| {
            let id_a = a.pointer("/resource/id").and_then(|v| v.as_str()).unwrap_or("");
            let id_b = b.pointer("/resource/id").and_then(|v| v.as_str()).unwrap_or("");
            id_a.cmp(id_b)
        });

        let total = entries.len() as u32;

        Ok(serde_json::json!({
            "resourceType": "Bundle",
            "type": "searchset",
            "total": total,
            "entry": entries
        }))
    }
}

/// Simple parameter matching for mock search.
/// Matches top-level fields or known FHIR search parameter patterns.
fn matches_params(resource: &serde_json::Value, params: &[(&str, &str)]) -> bool {
    for (key, value) in params {
        // Handle common FHIR search patterns
        let matched = match *key {
            // subject reference: "Patient/xxx"
            "subject" => resource
                .pointer("/subject/reference")
                .and_then(|v| v.as_str())
                .map_or(false, |r| r == *value || r.ends_with(&format!("/{}", value))),
            // patient reference (alias for subject in some resources)
            "patient" => resource
                .pointer("/subject/reference")
                .and_then(|v| v.as_str())
                .map_or(false, |r| {
                    r == format!("Patient/{}", value) || r == *value
                }),
            // Simple top-level field match
            _ => resource
                .get(*key)
                .and_then(|v| v.as_str())
                .map_or(false, |v| v == *value),
        };

        if !matched {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_crud() {
        let mock = MockFhirServer::new();

        // Create
        let patient = serde_json::json!({
            "resourceType": "Patient",
            "name": [{"family": "Test", "given": ["Mock"]}]
        });
        let created = mock.create_resource("Patient", &patient).await.unwrap();
        assert!(created.get("id").is_some());

        let id = created["id"].as_str().unwrap();

        // Read
        let read = mock.read_resource("Patient", id).await.unwrap();
        assert_eq!(read["name"][0]["family"], "Test");

        // Update
        let mut updated = read.clone();
        updated["name"][0]["family"] = serde_json::json!("Updated");
        let result = mock.update_resource("Patient", id, &updated).await.unwrap();
        assert_eq!(result["name"][0]["family"], "Updated");

        // Search
        let bundle = mock.search_resources("Patient", &[]).await.unwrap();
        assert_eq!(bundle["total"], 1);
    }

    #[tokio::test]
    async fn test_mock_not_found() {
        let mock = MockFhirServer::new();
        let err = mock.read_resource("Patient", "nonexistent").await;
        assert!(matches!(err, Err(FhirError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_mock_seed() {
        let mock = MockFhirServer::new();
        mock.seed(serde_json::json!({
            "resourceType": "Patient",
            "id": "patient-001",
            "name": [{"family": "Mitchell", "given": ["Sarah"]}]
        }))
        .await;

        let patient = mock.read_resource("Patient", "patient-001").await.unwrap();
        assert_eq!(patient["name"][0]["family"], "Mitchell");
    }

    #[tokio::test]
    async fn test_mock_search_with_params() {
        let mock = MockFhirServer::new();
        mock.seed(serde_json::json!({
            "resourceType": "Encounter",
            "id": "enc-001",
            "status": "in-progress",
            "subject": {"reference": "Patient/patient-001"}
        }))
        .await;
        mock.seed(serde_json::json!({
            "resourceType": "Encounter",
            "id": "enc-002",
            "status": "finished",
            "subject": {"reference": "Patient/patient-002"}
        }))
        .await;

        // Search by status
        let bundle = mock
            .search_resources("Encounter", &[("status", "in-progress")])
            .await
            .unwrap();
        assert_eq!(bundle["total"], 1);
        assert_eq!(bundle["entry"][0]["resource"]["id"], "enc-001");

        // Search by patient
        let bundle = mock
            .search_resources("Encounter", &[("patient", "patient-002")])
            .await
            .unwrap();
        assert_eq!(bundle["total"], 1);
        assert_eq!(bundle["entry"][0]["resource"]["id"], "enc-002");
    }
}
