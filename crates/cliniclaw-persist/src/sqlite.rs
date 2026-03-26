use chrono::DateTime;
use sqlx::sqlite::{SqliteConnectOptions, SqliteRow};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::{AuditEvent, PersistError};

pub struct SqliteAuditStore {
    pool: SqlitePool,
}

impl SqliteAuditStore {
    /// Returns a reference to the underlying connection pool.
    ///
    /// Used by the kernel's `SqliteWorkspaceStore` to share the same
    /// database connection (workspaces and turns live alongside audit_events).
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn new(database_url: &str) -> Result<Self, PersistError> {
        let options: SqliteConnectOptions = database_url.parse::<SqliteConnectOptions>()?
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options).await?;
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
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_decision ON audit_events(policy_decision)")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Atomically chain-link and append an audit event.
    ///
    /// Uses a SQLite IMMEDIATE transaction to atomically read the latest hash,
    /// recompute the event's chain fields, and insert. This eliminates TOCTOU
    /// races — the caller does NOT need to pre-fetch `latest_hash`.
    ///
    /// The event's `previous_hash` and `event_hash` are recomputed inside the
    /// transaction to guarantee they reflect the actual chain state.
    pub async fn append(&self, event: &mut AuditEvent) -> Result<(), PersistError> {
        let metadata_json = event
            .metadata
            .as_ref()
            .map(|m| serde_json::to_string(m))
            .transpose()?;

        let mut tx = self.pool.begin().await?;

        let latest: Option<(String,)> = sqlx::query_as(
            "SELECT event_hash FROM audit_events ORDER BY timestamp DESC, rowid DESC LIMIT 1",
        )
        .fetch_optional(&mut *tx)
        .await?;

        let latest_hash = latest.map(|r| r.0).unwrap_or_default();

        // Atomically assign the correct previous_hash and recompute event_hash
        event.previous_hash = latest_hash;
        event.event_hash = AuditEvent::compute_hash(
            &event.id,
            &event.timestamp,
            &event.actor_id,
            &event.patient_id,
            &event.action,
            &event.policy_decision,
            &event.input_hash,
            &event.output_hash,
            &event.previous_hash,
        );

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

    /// Query audit events by policy decision (outcome type).
    /// Useful for monitoring denial rates and failure patterns.
    pub async fn get_by_outcome(&self, decision: &str) -> Result<Vec<AuditEvent>, PersistError> {
        let rows: Vec<SqliteRow> = sqlx::query(
            "SELECT id, timestamp, actor_id, patient_id, action, policy_decision, input_hash, output_hash, previous_hash, event_hash, metadata
             FROM audit_events WHERE policy_decision = ? ORDER BY timestamp ASC",
        )
        .bind(decision)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_event).collect()
    }

    /// Count events grouped by policy decision. Returns a map of decision -> count.
    pub async fn outcome_counts(&self) -> Result<std::collections::HashMap<String, i64>, PersistError> {
        let rows: Vec<SqliteRow> = sqlx::query(
            "SELECT policy_decision, COUNT(*) as cnt FROM audit_events GROUP BY policy_decision",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut counts = std::collections::HashMap::new();
        for row in rows {
            let decision: String = Row::get(&row, "policy_decision");
            let cnt: i64 = Row::get(&row, "cnt");
            counts.insert(decision, cnt);
        }
        Ok(counts)
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
            // Check chain linkage
            if event.previous_hash != expected_previous {
                tracing::warn!(
                    event_id = %event.id,
                    expected = %expected_previous,
                    actual = %event.previous_hash,
                    "chain linkage broken"
                );
                return Ok(false);
            }
            // Recompute hash to detect field tampering
            if !event.verify_hash() {
                tracing::warn!(
                    event_id = %event.id,
                    "event hash does not match recomputed hash — possible tampering"
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
    use crate::{AuditEvent, AuditOutcome};

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
        let mut event = AuditEvent::new(
            "practitioner-1",
            Some("patient-1".to_string()),
            "ambient_note_generated",
            "allow",
            "input-hash-abc",
            "output-hash-def",
            "",
        );
        let event_id = event.id;

        store.append(&mut event).await.unwrap();

        let fetched = store.get(event_id).await.unwrap().expect("event should exist");
        assert_eq!(fetched.id, event_id);
        assert_eq!(fetched.action, "ambient_note_generated");
        assert_eq!(fetched.actor_id, "practitioner-1");
        // Verify tamper detection works on fetched events
        assert!(fetched.verify_hash());
    }

    #[tokio::test]
    async fn chain_of_two_events() {
        let store = test_store().await;

        let mut e1 = AuditEvent::new("actor-1", None, "action_1", "allow", "ih1", "oh1", "");
        store.append(&mut e1).await.unwrap();

        // Append auto-assigns previous_hash, no need to pre-fetch
        let mut e2 = AuditEvent::new("actor-1", None, "action_2", "allow", "ih2", "oh2", "");
        store.append(&mut e2).await.unwrap();

        assert!(store.verify_chain().await.unwrap());
    }

    #[tokio::test]
    async fn get_by_patient() {
        let store = test_store().await;

        let mut e1 = AuditEvent::new("actor-1", Some("patient-A".to_string()), "a1", "allow", "ih", "oh", "");
        store.append(&mut e1).await.unwrap();

        let mut e2 = AuditEvent::new("actor-1", Some("patient-B".to_string()), "a2", "allow", "ih", "oh", "");
        store.append(&mut e2).await.unwrap();

        let patient_a_events = store.get_by_patient("patient-A").await.unwrap();
        assert_eq!(patient_a_events.len(), 1);
        assert_eq!(patient_a_events[0].action, "a1");
    }

    #[tokio::test]
    async fn get_by_action() {
        let store = test_store().await;

        let mut e1 = AuditEvent::new("actor-1", None, "note_gen", "allow", "ih", "oh", "");
        store.append(&mut e1).await.unwrap();

        let mut e2 = AuditEvent::new("actor-1", None, "order_prop", "allow", "ih", "oh", "");
        store.append(&mut e2).await.unwrap();

        let note_events = store.get_by_action("note_gen").await.unwrap();
        assert_eq!(note_events.len(), 1);
    }

    #[tokio::test]
    async fn verify_chain_empty_store() {
        let store = test_store().await;
        assert!(store.verify_chain().await.unwrap());
    }

    #[tokio::test]
    async fn audit_policy_denial() {
        let store = test_store().await;
        let outcome = AuditOutcome::PolicyDenied {
            reason: "missing capability: order_entry".into(),
        };
        let mut event = AuditEvent::for_outcome(
            "practitioner-1",
            Some("patient-1".to_string()),
            "order_entry.propose",
            &outcome,
            "input-hash",
            "",
        );
        store.append(&mut event).await.unwrap();

        let denials = store.get_by_outcome("deny").await.unwrap();
        assert_eq!(denials.len(), 1);
        assert_eq!(denials[0].policy_decision, "deny");
        assert!(denials[0].metadata.is_some());
    }

    #[tokio::test]
    async fn audit_agent_error() {
        let store = test_store().await;
        let outcome = AuditOutcome::AgentError {
            reason: "LLM timeout".into(),
        };
        let mut event = AuditEvent::for_outcome(
            "practitioner-1",
            None,
            "ambient_doc.generate_note",
            &outcome,
            "input-hash",
            "",
        );
        store.append(&mut event).await.unwrap();

        let errors = store.get_by_outcome("agent_error").await.unwrap();
        assert_eq!(errors.len(), 1);
    }

    #[tokio::test]
    async fn outcome_counts_aggregation() {
        let store = test_store().await;

        // Mix of outcomes
        let mut e1 = AuditEvent::new("a", None, "act1", "allow", "ih", "oh", "");
        store.append(&mut e1).await.unwrap();

        let mut e2 = AuditEvent::for_outcome("a", None, "act2", &AuditOutcome::PolicyDenied { reason: "test".into() }, "ih", "");
        store.append(&mut e2).await.unwrap();

        let mut e3 = AuditEvent::new("a", None, "act3", "allow", "ih", "oh", "");
        store.append(&mut e3).await.unwrap();

        let counts = store.outcome_counts().await.unwrap();
        assert_eq!(counts.get("allow"), Some(&2));
        assert_eq!(counts.get("deny"), Some(&1));
    }

    #[tokio::test]
    async fn append_denial_event() {
        let store = test_store().await;
        let mut event = AuditEvent::denied(
            "practitioner-1",
            Some("patient-1".to_string()),
            "order_entry.propose",
            "missing capability: order_entry",
            "input-hash-abc",
        );
        store.append(&mut event).await.unwrap();

        let fetched = store.get(event.id).await.unwrap().expect("event should exist");
        assert_eq!(fetched.policy_decision, "policy_denied");
        assert_eq!(fetched.output_hash, "none");
        assert!(fetched.verify_hash());
        let meta = fetched.metadata.unwrap();
        assert_eq!(meta["outcome"], "policy_denied");
        assert_eq!(meta["reason"], "missing capability: order_entry");
    }

    #[tokio::test]
    async fn append_approval_event() {
        let store = test_store().await;
        let mut event = AuditEvent::awaiting_approval(
            "practitioner-1",
            Some("patient-1".to_string()),
            "order_entry.propose_controlled",
            "input-hash-abc",
        );
        store.append(&mut event).await.unwrap();

        let fetched = store.get(event.id).await.unwrap().expect("event should exist");
        assert_eq!(fetched.policy_decision, "awaiting_approval");
        assert!(fetched.verify_hash());
    }

    #[tokio::test]
    async fn append_agent_error_event() {
        let store = test_store().await;
        let mut event = AuditEvent::agent_error(
            "triage-agent",
            Some("patient-1".to_string()),
            "triage_assess.evaluate",
            "LLM timeout after 120s",
            "input-hash-abc",
        );
        store.append(&mut event).await.unwrap();

        let fetched = store.get(event.id).await.unwrap().expect("event should exist");
        assert_eq!(fetched.policy_decision, "agent_error");
        let meta = fetched.metadata.unwrap();
        assert_eq!(meta["error"], "LLM timeout after 120s");
    }

    #[tokio::test]
    async fn append_verification_failed_event() {
        let store = test_store().await;
        let mut event = AuditEvent::verification_failed(
            "ambient-doc-agent",
            Some("patient-1".to_string()),
            "ambient_doc.generate_note",
            "output missing required SOAP section",
            "input-hash-abc",
            "output-hash-bad",
        );
        store.append(&mut event).await.unwrap();

        let fetched = store.get(event.id).await.unwrap().expect("event should exist");
        assert_eq!(fetched.policy_decision, "verification_failed");
        assert_eq!(fetched.output_hash, "output-hash-bad");
    }

    #[tokio::test]
    async fn mixed_outcome_chain_verifies() {
        let store = test_store().await;

        let mut e1 = AuditEvent::denied("a", None, "act1", "no cap", "ih1");
        store.append(&mut e1).await.unwrap();

        let mut e2 = AuditEvent::new("a", None, "act2", "allow", "ih2", "oh2", "");
        store.append(&mut e2).await.unwrap();

        let mut e3 = AuditEvent::agent_error("a", None, "act3", "timeout", "ih3");
        store.append(&mut e3).await.unwrap();

        assert!(store.verify_chain().await.unwrap());
    }

    #[tokio::test]
    async fn failure_events_chain_correctly() {
        let store = test_store().await;

        let mut e1 = AuditEvent::new("a", None, "act1", "allow", "ih", "oh", "");
        store.append(&mut e1).await.unwrap();

        let mut e2 = AuditEvent::for_outcome("a", None, "act2", &AuditOutcome::PolicyDenied { reason: "denied".into() }, "ih", "");
        store.append(&mut e2).await.unwrap();

        let mut e3 = AuditEvent::for_outcome("a", None, "act3", &AuditOutcome::AgentError { reason: "timeout".into() }, "ih", "");
        store.append(&mut e3).await.unwrap();

        // Chain should be valid even with mixed success/failure events
        assert!(store.verify_chain().await.unwrap());
    }
}
