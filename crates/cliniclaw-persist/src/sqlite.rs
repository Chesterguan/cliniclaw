use chrono::DateTime;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::{AuditEvent, PersistError};

pub struct SqliteAuditStore {
    pool: SqlitePool,
}

impl SqliteAuditStore {
    pub async fn new(database_url: &str) -> Result<Self, PersistError> {
        let pool = SqlitePool::connect(database_url).await?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    async fn migrate(&self) -> Result<(), PersistError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS audit_events (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                actor_id TEXT NOT NULL,
                patient_id TEXT,
                action TEXT NOT NULL,
                policy_decision TEXT NOT NULL,
                input_hash TEXT NOT NULL,
                output_hash TEXT NOT NULL,
                previous_hash TEXT NOT NULL,
                event_hash TEXT NOT NULL,
                metadata TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_events(timestamp)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_patient ON audit_events(patient_id)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_events(action)")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Atomically verify chain integrity and append. Uses BEGIN IMMEDIATE
    /// to acquire a write lock before reading latest_hash, preventing TOCTOU races.
    pub async fn append(&self, event: &AuditEvent) -> Result<(), PersistError> {
        let metadata_json = event
            .metadata
            .as_ref()
            .map(|m| serde_json::to_string(m))
            .transpose()?;

        let mut tx = self.pool.begin().await?;

        // BEGIN IMMEDIATE is the default for sqlx sqlite write transactions —
        // this holds the write lock for the duration of the transaction.
        let latest: Option<(String,)> = sqlx::query_as(
            "SELECT event_hash FROM audit_events ORDER BY timestamp DESC, rowid DESC LIMIT 1",
        )
        .fetch_optional(&mut *tx)
        .await?;

        let latest_hash = latest.map(|r| r.0).unwrap_or_default();
        if event.previous_hash != latest_hash {
            return Err(PersistError::ChainIntegrity {
                expected: latest_hash,
                actual: event.previous_hash.clone(),
            });
        }

        sqlx::query(
            "INSERT INTO audit_events (id, timestamp, actor_id, patient_id, action, policy_decision, input_hash, output_hash, previous_hash, event_hash, metadata)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(event.id.to_string())
        .bind(event.timestamp.to_rfc3339())
        .bind(&event.actor_id)
        .bind(&event.patient_id)
        .bind(&event.action)
        .bind(&event.policy_decision)
        .bind(&event.input_hash)
        .bind(&event.output_hash)
        .bind(&event.previous_hash)
        .bind(&event.event_hash)
        .bind(metadata_json)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        tracing::info!(
            event_id = %event.id,
            action = %event.action,
            actor = %event.actor_id,
            "audit event appended"
        );

        Ok(())
    }

    pub async fn latest_hash(&self) -> Result<String, PersistError> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT event_hash FROM audit_events ORDER BY timestamp DESC, rowid DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.0).unwrap_or_default())
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<AuditEvent>, PersistError> {
        let row: Option<SqliteRow> = sqlx::query(
            "SELECT id, timestamp, actor_id, patient_id, action, policy_decision, input_hash, output_hash, previous_hash, event_hash, metadata
             FROM audit_events WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_event(r)).transpose()
    }

    pub async fn get_by_patient(&self, patient_id: &str) -> Result<Vec<AuditEvent>, PersistError> {
        let rows: Vec<SqliteRow> = sqlx::query(
            "SELECT id, timestamp, actor_id, patient_id, action, policy_decision, input_hash, output_hash, previous_hash, event_hash, metadata
             FROM audit_events WHERE patient_id = ? ORDER BY timestamp ASC",
        )
        .bind(patient_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_event).collect()
    }

    pub async fn get_by_action(&self, action: &str) -> Result<Vec<AuditEvent>, PersistError> {
        let rows: Vec<SqliteRow> = sqlx::query(
            "SELECT id, timestamp, actor_id, patient_id, action, policy_decision, input_hash, output_hash, previous_hash, event_hash, metadata
             FROM audit_events WHERE action = ? ORDER BY timestamp ASC",
        )
        .bind(action)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_event).collect()
    }

    pub async fn verify_chain(&self) -> Result<bool, PersistError> {
        let rows: Vec<SqliteRow> = sqlx::query(
            "SELECT id, timestamp, actor_id, patient_id, action, policy_decision, input_hash, output_hash, previous_hash, event_hash, metadata
             FROM audit_events ORDER BY timestamp ASC, rowid ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        let events: Vec<AuditEvent> = rows.into_iter().map(row_to_event).collect::<Result<_, _>>()?;

        let mut expected_previous = String::new();
        for event in &events {
            if event.previous_hash != expected_previous {
                tracing::warn!(
                    event_id = %event.id,
                    expected = %expected_previous,
                    actual = %event.previous_hash,
                    "chain integrity broken"
                );
                return Ok(false);
            }
            expected_previous = event.event_hash.clone();
        }

        Ok(true)
    }
}

fn row_to_event(row: SqliteRow) -> Result<AuditEvent, PersistError> {
    let id_str: String = Row::get(&row, "id");
    let id = Uuid::parse_str(&id_str)
        .map_err(|e| PersistError::Corrupt(format!("invalid UUID '{}': {}", id_str, e)))?;

    let ts_str: String = Row::get(&row, "timestamp");
    let timestamp = DateTime::parse_from_rfc3339(&ts_str)
        .map_err(|e| PersistError::Corrupt(format!("invalid timestamp '{}': {}", ts_str, e)))?
        .with_timezone(&chrono::Utc);

    let metadata_str: Option<String> = Row::get(&row, "metadata");
    let metadata = metadata_str
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?;

    Ok(AuditEvent {
        id,
        timestamp,
        actor_id: Row::get(&row, "actor_id"),
        patient_id: Row::get(&row, "patient_id"),
        action: Row::get(&row, "action"),
        policy_decision: Row::get(&row, "policy_decision"),
        input_hash: Row::get(&row, "input_hash"),
        output_hash: Row::get(&row, "output_hash"),
        previous_hash: Row::get(&row, "previous_hash"),
        event_hash: Row::get(&row, "event_hash"),
        metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AuditEvent;

    async fn test_store() -> SqliteAuditStore {
        SqliteAuditStore::new("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn empty_store_latest_hash_is_empty() {
        let store = test_store().await;
        assert_eq!(store.latest_hash().await.unwrap(), "");
    }

    #[tokio::test]
    async fn append_and_get() {
        let store = test_store().await;
        let event = AuditEvent::new(
            "practitioner-1",
            Some("patient-1".to_string()),
            "ambient_note_generated",
            "allow",
            "input-hash-abc",
            "output-hash-def",
            "",
        );
        let event_id = event.id;

        store.append(&event).await.unwrap();

        let fetched = store.get(event_id).await.unwrap().expect("event should exist");
        assert_eq!(fetched.id, event_id);
        assert_eq!(fetched.action, "ambient_note_generated");
        assert_eq!(fetched.actor_id, "practitioner-1");
    }

    #[tokio::test]
    async fn chain_integrity_violation() {
        let store = test_store().await;
        let event = AuditEvent::new(
            "practitioner-1",
            None,
            "test_action",
            "allow",
            "ih",
            "oh",
            "wrong-previous-hash",
        );

        let result = store.append(&event).await;
        assert!(matches!(result, Err(PersistError::ChainIntegrity { .. })));
    }

    #[tokio::test]
    async fn chain_of_two_events() {
        let store = test_store().await;

        let e1 = AuditEvent::new("actor-1", None, "action_1", "allow", "ih1", "oh1", "");
        store.append(&e1).await.unwrap();

        let latest = store.latest_hash().await.unwrap();
        assert_eq!(latest, e1.event_hash);

        let e2 = AuditEvent::new("actor-1", None, "action_2", "allow", "ih2", "oh2", &latest);
        store.append(&e2).await.unwrap();

        assert!(store.verify_chain().await.unwrap());
    }

    #[tokio::test]
    async fn get_by_patient() {
        let store = test_store().await;

        let e1 = AuditEvent::new("actor-1", Some("patient-A".to_string()), "a1", "allow", "ih", "oh", "");
        store.append(&e1).await.unwrap();

        let e2 = AuditEvent::new("actor-1", Some("patient-B".to_string()), "a2", "allow", "ih", "oh", &e1.event_hash);
        store.append(&e2).await.unwrap();

        let patient_a_events = store.get_by_patient("patient-A").await.unwrap();
        assert_eq!(patient_a_events.len(), 1);
        assert_eq!(patient_a_events[0].action, "a1");
    }

    #[tokio::test]
    async fn get_by_action() {
        let store = test_store().await;

        let e1 = AuditEvent::new("actor-1", None, "note_gen", "allow", "ih", "oh", "");
        store.append(&e1).await.unwrap();

        let e2 = AuditEvent::new("actor-1", None, "order_prop", "allow", "ih", "oh", &e1.event_hash);
        store.append(&e2).await.unwrap();

        let note_events = store.get_by_action("note_gen").await.unwrap();
        assert_eq!(note_events.len(), 1);
    }

    #[tokio::test]
    async fn verify_chain_empty_store() {
        let store = test_store().await;
        assert!(store.verify_chain().await.unwrap());
    }
}
