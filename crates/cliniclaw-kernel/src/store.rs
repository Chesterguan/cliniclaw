use async_trait::async_trait;
use chrono::Utc;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use crate::error::KernelError;
use crate::types::*;

/// Persistence layer for workspaces and turns.
#[async_trait]
pub trait WorkspaceStore: Send + Sync + std::fmt::Debug {
    async fn create_workspace(
        &self,
        encounter_id: &str,
        practitioner_id: &str,
    ) -> Result<Workspace, KernelError>;

    async fn get_workspace(&self, id: &str) -> Result<Workspace, KernelError>;

    /// Find an open workspace for the given encounter, if one exists.
    async fn find_workspace_by_encounter(
        &self,
        encounter_id: &str,
    ) -> Result<Option<Workspace>, KernelError>;

    async fn close_workspace(&self, id: &str) -> Result<Workspace, KernelError>;

    async fn create_turn(&self, turn: &Turn) -> Result<(), KernelError>;

    async fn get_turn(&self, id: &str) -> Result<Turn, KernelError>;

    async fn list_turns(
        &self,
        workspace_id: &str,
        status_filter: Option<TurnStatus>,
    ) -> Result<Vec<Turn>, KernelError>;

    async fn resolve_turn(
        &self,
        id: &str,
        status: TurnStatus,
        feedback: Option<Feedback>,
        resolved_by: &str,
    ) -> Result<Turn, KernelError>;

    async fn get_replay_input(&self, turn_id: &str) -> Result<ReplayInput, KernelError>;

    /// Feedback statistics for an agent over a time range.
    async fn get_feedback_stats(
        &self,
        agent_name: Option<&str>,
    ) -> Result<FeedbackStats, KernelError>;
}

/// Aggregate feedback statistics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FeedbackStats {
    pub total_turns: i64,
    pub accepted: i64,
    pub modified: i64,
    pub rejected: i64,
    pub escalated: i64,
    pub pending: i64,
    pub avg_confidence: f64,
}

/// SQLite implementation of the workspace store.
#[derive(Debug)]
pub struct SqliteWorkspaceStore {
    pool: SqlitePool,
}

impl SqliteWorkspaceStore {
    /// Create a new store using an existing pool (shared with audit store).
    pub async fn new(pool: SqlitePool) -> Result<Self, KernelError> {
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    async fn migrate(&self) -> Result<(), KernelError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS workspaces (
                id TEXT PRIMARY KEY,
                encounter_id TEXT NOT NULL,
                practitioner_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                closed_at TEXT
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_workspaces_encounter ON workspaces(encounter_id)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS turns (
                id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL REFERENCES workspaces(id),
                agent_name TEXT NOT NULL,
                action TEXT NOT NULL,
                input_snapshot TEXT NOT NULL,
                output_snapshot TEXT NOT NULL,
                confidence_score REAL NOT NULL,
                confidence_factors TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                feedback_action TEXT,
                feedback_original TEXT,
                feedback_corrected TEXT,
                feedback_reason TEXT,
                feedback_timestamp TEXT,
                created_at TEXT NOT NULL,
                resolved_at TEXT,
                resolved_by TEXT,
                triggered_by_turn_id TEXT
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_turns_workspace ON turns(workspace_id)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_turns_status ON turns(status)",
        )
        .execute(&self.pool)
        .await?;

        // Migration: add triggered_by_turn_id column if it doesn't exist
        let _ = sqlx::query("ALTER TABLE turns ADD COLUMN triggered_by_turn_id TEXT")
            .execute(&self.pool)
            .await;

        Ok(())
    }
}

#[async_trait]
impl WorkspaceStore for SqliteWorkspaceStore {
    async fn create_workspace(
        &self,
        encounter_id: &str,
        practitioner_id: &str,
    ) -> Result<Workspace, KernelError> {
        let ws = Workspace {
            id: uuid::Uuid::new_v4().to_string(),
            encounter_id: encounter_id.to_string(),
            practitioner_id: practitioner_id.to_string(),
            created_at: Utc::now(),
            closed_at: None,
        };

        sqlx::query(
            "INSERT INTO workspaces (id, encounter_id, practitioner_id, created_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(&ws.id)
        .bind(&ws.encounter_id)
        .bind(&ws.practitioner_id)
        .bind(ws.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(ws)
    }

    async fn get_workspace(&self, id: &str) -> Result<Workspace, KernelError> {
        let row: SqliteRow = sqlx::query(
            "SELECT id, encounter_id, practitioner_id, created_at, closed_at
             FROM workspaces WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| KernelError::WorkspaceNotFound(id.to_string()))?;

        row_to_workspace(row)
    }

    async fn find_workspace_by_encounter(
        &self,
        encounter_id: &str,
    ) -> Result<Option<Workspace>, KernelError> {
        let row: Option<SqliteRow> = sqlx::query(
            "SELECT id, encounter_id, practitioner_id, created_at, closed_at
             FROM workspaces WHERE encounter_id = ? AND closed_at IS NULL
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(encounter_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(row_to_workspace).transpose()
    }

    async fn close_workspace(&self, id: &str) -> Result<Workspace, KernelError> {
        let ws = self.get_workspace(id).await?;
        if ws.closed_at.is_some() {
            return Err(KernelError::WorkspaceClosed(id.to_string()));
        }

        let now = Utc::now();
        sqlx::query("UPDATE workspaces SET closed_at = ? WHERE id = ?")
            .bind(now.to_rfc3339())
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(Workspace {
            closed_at: Some(now),
            ..ws
        })
    }

    async fn create_turn(&self, turn: &Turn) -> Result<(), KernelError> {
        let confidence_factors = serde_json::to_string(&turn.confidence.factors)?;
        let input_json = serde_json::to_string(&turn.input_snapshot)?;
        let output_json = serde_json::to_string(&turn.output_snapshot)?;

        sqlx::query(
            "INSERT INTO turns (id, workspace_id, agent_name, action, input_snapshot, output_snapshot,
             confidence_score, confidence_factors, status, created_at, triggered_by_turn_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&turn.id)
        .bind(&turn.workspace_id)
        .bind(&turn.agent_name)
        .bind(&turn.action)
        .bind(&input_json)
        .bind(&output_json)
        .bind(turn.confidence.score)
        .bind(&confidence_factors)
        .bind(turn.status.to_string())
        .bind(turn.created_at.to_rfc3339())
        .bind(&turn.triggered_by_turn_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_turn(&self, id: &str) -> Result<Turn, KernelError> {
        let row: SqliteRow = sqlx::query(
            "SELECT id, workspace_id, agent_name, action, input_snapshot, output_snapshot,
             confidence_score, confidence_factors, status, feedback_action, feedback_original,
             feedback_corrected, feedback_reason, feedback_timestamp, created_at,
             resolved_at, resolved_by
             FROM turns WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| KernelError::TurnNotFound(id.to_string()))?;

        row_to_turn(row)
    }

    async fn list_turns(
        &self,
        workspace_id: &str,
        status_filter: Option<TurnStatus>,
    ) -> Result<Vec<Turn>, KernelError> {
        let rows: Vec<SqliteRow> = if let Some(status) = status_filter {
            sqlx::query(
                "SELECT id, workspace_id, agent_name, action, input_snapshot, output_snapshot,
                 confidence_score, confidence_factors, status, feedback_action, feedback_original,
                 feedback_corrected, feedback_reason, feedback_timestamp, created_at,
                 resolved_at, resolved_by
                 FROM turns WHERE workspace_id = ? AND status = ? ORDER BY created_at ASC",
            )
            .bind(workspace_id)
            .bind(status.to_string())
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, workspace_id, agent_name, action, input_snapshot, output_snapshot,
                 confidence_score, confidence_factors, status, feedback_action, feedback_original,
                 feedback_corrected, feedback_reason, feedback_timestamp, created_at,
                 resolved_at, resolved_by
                 FROM turns WHERE workspace_id = ? ORDER BY created_at ASC",
            )
            .bind(workspace_id)
            .fetch_all(&self.pool)
            .await?
        };

        rows.into_iter().map(row_to_turn).collect()
    }

    async fn resolve_turn(
        &self,
        id: &str,
        status: TurnStatus,
        feedback: Option<Feedback>,
        resolved_by: &str,
    ) -> Result<Turn, KernelError> {
        let existing = self.get_turn(id).await?;
        if existing.status != TurnStatus::Pending {
            return Err(KernelError::InvalidTransition {
                from: existing.status.to_string(),
                to: status.to_string(),
            });
        }

        let now = Utc::now();

        let (fb_action, fb_original, fb_corrected, fb_reason, fb_timestamp) =
            if let Some(ref fb) = feedback {
                (
                    Some(fb.action.to_string()),
                    Some(serde_json::to_string(&fb.original_output)?),
                    fb.corrected_output
                        .as_ref()
                        .map(|v| serde_json::to_string(v))
                        .transpose()?,
                    fb.reason.clone(),
                    Some(fb.timestamp.to_rfc3339()),
                )
            } else {
                (None, None, None, None, None)
            };

        sqlx::query(
            "UPDATE turns SET status = ?, feedback_action = ?, feedback_original = ?,
             feedback_corrected = ?, feedback_reason = ?, feedback_timestamp = ?,
             resolved_at = ?, resolved_by = ?
             WHERE id = ?",
        )
        .bind(status.to_string())
        .bind(&fb_action)
        .bind(&fb_original)
        .bind(&fb_corrected)
        .bind(&fb_reason)
        .bind(&fb_timestamp)
        .bind(now.to_rfc3339())
        .bind(resolved_by)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get_turn(id).await
    }

    async fn get_replay_input(&self, turn_id: &str) -> Result<ReplayInput, KernelError> {
        let turn = self.get_turn(turn_id).await?;
        Ok(ReplayInput {
            turn_id: turn.id,
            agent_name: turn.agent_name,
            input_snapshot: turn.input_snapshot,
            original_output: turn.output_snapshot,
        })
    }

    async fn get_feedback_stats(
        &self,
        agent_name: Option<&str>,
    ) -> Result<FeedbackStats, KernelError> {
        let row: SqliteRow = if let Some(agent) = agent_name {
            sqlx::query(
                "SELECT
                    COUNT(*) as total,
                    SUM(CASE WHEN status = 'accepted' THEN 1 ELSE 0 END) as accepted,
                    SUM(CASE WHEN status = 'modified' THEN 1 ELSE 0 END) as modified,
                    SUM(CASE WHEN status = 'rejected' THEN 1 ELSE 0 END) as rejected,
                    SUM(CASE WHEN status = 'escalated' THEN 1 ELSE 0 END) as escalated,
                    SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END) as pending,
                    AVG(confidence_score) as avg_confidence
                 FROM turns WHERE agent_name = ?",
            )
            .bind(agent)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT
                    COUNT(*) as total,
                    SUM(CASE WHEN status = 'accepted' THEN 1 ELSE 0 END) as accepted,
                    SUM(CASE WHEN status = 'modified' THEN 1 ELSE 0 END) as modified,
                    SUM(CASE WHEN status = 'rejected' THEN 1 ELSE 0 END) as rejected,
                    SUM(CASE WHEN status = 'escalated' THEN 1 ELSE 0 END) as escalated,
                    SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END) as pending,
                    AVG(confidence_score) as avg_confidence
                 FROM turns",
            )
            .fetch_one(&self.pool)
            .await?
        };

        Ok(FeedbackStats {
            total_turns: Row::get(&row, "total"),
            accepted: Row::get::<i64, _>(&row, "accepted"),
            modified: Row::get::<i64, _>(&row, "modified"),
            rejected: Row::get::<i64, _>(&row, "rejected"),
            escalated: Row::get::<i64, _>(&row, "escalated"),
            pending: Row::get::<i64, _>(&row, "pending"),
            avg_confidence: Row::get::<f64, _>(&row, "avg_confidence"),
        })
    }
}

fn row_to_workspace(row: SqliteRow) -> Result<Workspace, KernelError> {
    let created_str: String = Row::get(&row, "created_at");
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_str)
        .map_err(|e| KernelError::Corrupt(e.to_string()))?
        .with_timezone(&Utc);

    let closed_str: Option<String> = Row::get(&row, "closed_at");
    let closed_at = closed_str
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| KernelError::Corrupt(e.to_string()))
        })
        .transpose()?;

    Ok(Workspace {
        id: Row::get(&row, "id"),
        encounter_id: Row::get(&row, "encounter_id"),
        practitioner_id: Row::get(&row, "practitioner_id"),
        created_at,
        closed_at,
    })
}

fn row_to_turn(row: SqliteRow) -> Result<Turn, KernelError> {
    let created_str: String = Row::get(&row, "created_at");
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_str)
        .map_err(|e| KernelError::Corrupt(e.to_string()))?
        .with_timezone(&Utc);

    let resolved_str: Option<String> = Row::get(&row, "resolved_at");
    let resolved_at = resolved_str
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| KernelError::Corrupt(e.to_string()))
        })
        .transpose()?;

    let status_str: String = Row::get(&row, "status");
    let status: TurnStatus = status_str
        .parse()
        .map_err(|e: String| KernelError::Corrupt(e))?;

    let confidence_factors_str: String = Row::get(&row, "confidence_factors");
    let confidence_factors: Vec<String> = serde_json::from_str(&confidence_factors_str)?;
    let confidence_score: f64 = Row::get(&row, "confidence_score");

    let input_str: String = Row::get(&row, "input_snapshot");
    let output_str: String = Row::get(&row, "output_snapshot");

    // Reconstruct feedback if present
    let fb_action_str: Option<String> = Row::get(&row, "feedback_action");
    let feedback = if let Some(action_str) = fb_action_str {
        let fb_original_str: Option<String> = Row::get(&row, "feedback_original");
        let fb_corrected_str: Option<String> = Row::get(&row, "feedback_corrected");
        let fb_reason: Option<String> = Row::get(&row, "feedback_reason");
        let fb_ts_str: Option<String> = Row::get(&row, "feedback_timestamp");

        let action = match action_str.as_str() {
            "accept" => FeedbackAction::Accept,
            "modify" => FeedbackAction::Modify,
            "reject" => FeedbackAction::Reject,
            "escalate" => FeedbackAction::Escalate,
            other => {
                return Err(KernelError::Corrupt(
                    format!("unknown feedback action: {other}"),
                ))
            }
        };

        let original_output = fb_original_str
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or(serde_json::Value::Null);

        let corrected_output = fb_corrected_str
            .map(|s| serde_json::from_str(&s))
            .transpose()?;

        let timestamp = fb_ts_str
            .map(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| {
                        KernelError::Corrupt(e.to_string())
                    })
            })
            .transpose()?
            .unwrap_or_else(Utc::now);

        Some(Feedback {
            action,
            original_output,
            corrected_output,
            reason: fb_reason,
            timestamp,
        })
    } else {
        None
    };

    let triggered_by: Option<String> = Row::try_get(&row, "triggered_by_turn_id").ok().flatten();

    Ok(Turn {
        id: Row::get(&row, "id"),
        workspace_id: Row::get(&row, "workspace_id"),
        agent_name: Row::get(&row, "agent_name"),
        action: Row::get(&row, "action"),
        input_snapshot: serde_json::from_str(&input_str)?,
        output_snapshot: serde_json::from_str(&output_str)?,
        confidence: Confidence {
            score: confidence_score,
            factors: confidence_factors,
        },
        status,
        feedback,
        created_at,
        resolved_at,
        resolved_by: Row::get(&row, "resolved_by"),
        triggered_by_turn_id: triggered_by,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_store() -> SqliteWorkspaceStore {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        SqliteWorkspaceStore::new(pool).await.unwrap()
    }

    #[tokio::test]
    async fn create_and_get_workspace() {
        let store = test_store().await;
        let ws = store
            .create_workspace("enc-001", "pract-001")
            .await
            .unwrap();
        assert_eq!(ws.encounter_id, "enc-001");
        assert!(ws.closed_at.is_none());

        let fetched = store.get_workspace(&ws.id).await.unwrap();
        assert_eq!(fetched.id, ws.id);
    }

    #[tokio::test]
    async fn find_workspace_by_encounter() {
        let store = test_store().await;
        let ws = store
            .create_workspace("enc-001", "pract-001")
            .await
            .unwrap();

        let found = store
            .find_workspace_by_encounter("enc-001")
            .await
            .unwrap();
        assert_eq!(found.unwrap().id, ws.id);

        let not_found = store
            .find_workspace_by_encounter("enc-999")
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn close_workspace() {
        let store = test_store().await;
        let ws = store
            .create_workspace("enc-001", "pract-001")
            .await
            .unwrap();

        let closed = store.close_workspace(&ws.id).await.unwrap();
        assert!(closed.closed_at.is_some());

        // Double-close should error
        let err = store.close_workspace(&ws.id).await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn create_and_resolve_turn() {
        let store = test_store().await;
        let ws = store
            .create_workspace("enc-001", "pract-001")
            .await
            .unwrap();

        let turn = Turn {
            id: uuid::Uuid::new_v4().to_string(),
            workspace_id: ws.id.clone(),
            agent_name: "ambient_doc".to_string(),
            action: "generate_note".to_string(),
            input_snapshot: serde_json::json!({"transcript": "patient says..."}),
            output_snapshot: serde_json::json!({"subjective": "Patient reports..."}),
            confidence: Confidence::high(vec!["complete_soap".into()]),
            status: TurnStatus::Pending,
            feedback: None,
            created_at: Utc::now(),
            resolved_at: None,
            resolved_by: None,
            triggered_by_turn_id: None,
        };

        store.create_turn(&turn).await.unwrap();

        // List pending turns
        let pending = store
            .list_turns(&ws.id, Some(TurnStatus::Pending))
            .await
            .unwrap();
        assert_eq!(pending.len(), 1);

        // Resolve turn with feedback
        let feedback = Feedback {
            action: FeedbackAction::Modify,
            original_output: serde_json::json!({"subjective": "Patient reports..."}),
            corrected_output: Some(serde_json::json!({"subjective": "Patient reports headache..."})),
            reason: Some("Added specifics".to_string()),
            timestamp: Utc::now(),
        };

        let resolved = store
            .resolve_turn(&turn.id, TurnStatus::Modified, Some(feedback), "pract-001")
            .await
            .unwrap();
        assert_eq!(resolved.status, TurnStatus::Modified);
        assert!(resolved.feedback.is_some());
        assert!(resolved.resolved_at.is_some());
        assert_eq!(resolved.resolved_by.as_deref(), Some("pract-001"));

        // Should not be able to resolve again
        let err = store
            .resolve_turn(&turn.id, TurnStatus::Accepted, None, "pract-001")
            .await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn replay_input() {
        let store = test_store().await;
        let ws = store
            .create_workspace("enc-001", "pract-001")
            .await
            .unwrap();

        let turn = Turn {
            id: uuid::Uuid::new_v4().to_string(),
            workspace_id: ws.id.clone(),
            agent_name: "order_entry".to_string(),
            action: "propose_order".to_string(),
            input_snapshot: serde_json::json!({"order_text": "metformin 500mg BID"}),
            output_snapshot: serde_json::json!({"medication": "metformin"}),
            confidence: Confidence::medium(vec!["known_medication".into()]),
            status: TurnStatus::Pending,
            feedback: None,
            created_at: Utc::now(),
            resolved_at: None,
            resolved_by: None,
            triggered_by_turn_id: None,
        };

        store.create_turn(&turn).await.unwrap();

        let replay = store.get_replay_input(&turn.id).await.unwrap();
        assert_eq!(replay.agent_name, "order_entry");
        assert_eq!(
            replay.input_snapshot,
            serde_json::json!({"order_text": "metformin 500mg BID"})
        );
    }

    #[tokio::test]
    async fn feedback_stats() {
        let store = test_store().await;
        let ws = store
            .create_workspace("enc-001", "pract-001")
            .await
            .unwrap();

        // Create 3 turns for ambient_doc
        for i in 0..3 {
            let turn = Turn {
                id: uuid::Uuid::new_v4().to_string(),
                workspace_id: ws.id.clone(),
                agent_name: "ambient_doc".to_string(),
                action: "generate_note".to_string(),
                input_snapshot: serde_json::json!({"i": i}),
                output_snapshot: serde_json::json!({"note": "..."}),
                confidence: Confidence::new(0.8, vec!["test".into()]),
                status: TurnStatus::Pending,
                feedback: None,
                created_at: Utc::now(),
                resolved_at: None,
                resolved_by: None,
                triggered_by_turn_id: None,
            };
            store.create_turn(&turn).await.unwrap();

            // Resolve first two (accept + modify)
            if i < 2 {
                let status = if i == 0 {
                    TurnStatus::Accepted
                } else {
                    TurnStatus::Modified
                };
                store
                    .resolve_turn(&turn.id, status, None, "pract-001")
                    .await
                    .unwrap();
            }
        }

        let stats = store
            .get_feedback_stats(Some("ambient_doc"))
            .await
            .unwrap();
        assert_eq!(stats.total_turns, 3);
        assert_eq!(stats.accepted, 1);
        assert_eq!(stats.modified, 1);
        assert_eq!(stats.pending, 1);
        assert!((stats.avg_confidence - 0.8).abs() < 0.01);
    }

    #[tokio::test]
    async fn workspace_not_found() {
        let store = test_store().await;
        let err = store.get_workspace("nonexistent").await;
        assert!(matches!(err, Err(KernelError::WorkspaceNotFound(_))));
    }

    #[tokio::test]
    async fn turn_not_found() {
        let store = test_store().await;
        let err = store.get_turn("nonexistent").await;
        assert!(matches!(err, Err(KernelError::TurnNotFound(_))));
    }
}
