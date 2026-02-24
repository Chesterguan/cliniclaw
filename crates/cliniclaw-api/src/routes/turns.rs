use std::sync::Arc;

use axum::{
    extract::{Json, Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use cliniclaw_kernel::{Feedback, FeedbackAction, TurnStatus};

use crate::error::ApiError;
use crate::state::AppState;
use super::extract_bearer_token;

#[derive(Debug, serde::Deserialize)]
pub struct ListTurnsQuery {
    pub status: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct TurnResponse {
    pub id: String,
    pub workspace_id: String,
    pub agent_name: String,
    pub action: String,
    pub output_snapshot: serde_json::Value,
    pub confidence: ConfidenceResponse,
    pub status: String,
    pub feedback: Option<FeedbackResponse>,
    pub created_at: String,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<String>,
    /// If this turn was triggered by another turn (agent chain), the source turn ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triggered_by_turn_id: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct ConfidenceResponse {
    pub score: f64,
    pub factors: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct FeedbackResponse {
    pub action: String,
    pub corrected_output: Option<serde_json::Value>,
    pub reason: Option<String>,
}

impl From<&cliniclaw_kernel::Turn> for TurnResponse {
    fn from(t: &cliniclaw_kernel::Turn) -> Self {
        TurnResponse {
            id: t.id.clone(),
            workspace_id: t.workspace_id.clone(),
            agent_name: t.agent_name.clone(),
            action: t.action.clone(),
            output_snapshot: t.output_snapshot.clone(),
            confidence: ConfidenceResponse {
                score: t.confidence.score,
                factors: t.confidence.factors.clone(),
            },
            status: t.status.to_string(),
            feedback: t.feedback.as_ref().map(|f| FeedbackResponse {
                action: f.action.to_string(),
                corrected_output: f.corrected_output.clone(),
                reason: f.reason.clone(),
            }),
            created_at: t.created_at.to_rfc3339(),
            resolved_at: t.resolved_at.map(|dt| dt.to_rfc3339()),
            resolved_by: t.resolved_by.clone(),
            triggered_by_turn_id: t.triggered_by_turn_id.clone(),
        }
    }
}

/// Returns the full chain of turns rooted at (or containing) the given turn.
pub async fn get_turn_chain(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Vec<TurnResponse>>, ApiError> {
    let _bearer = extract_bearer_token(&headers)?;

    // Start from the given turn
    let root = state
        .workspace_store
        .get_turn(&id)
        .await
        .map_err(ApiError::from)?;

    // Walk up to find the chain root (turn with no trigger)
    let mut root_id = root.id.clone();
    let mut current = root.clone();
    while let Some(ref parent_id) = current.triggered_by_turn_id {
        current = state
            .workspace_store
            .get_turn(parent_id)
            .await
            .map_err(ApiError::from)?;
        root_id = current.id.clone();
    }

    // Collect all turns and build lookup maps for O(1) access
    let all_turns = state
        .workspace_store
        .list_turns(&root.workspace_id, None)
        .await
        .map_err(ApiError::from)?;

    let turn_map: std::collections::HashMap<&str, &cliniclaw_kernel::Turn> =
        all_turns.iter().map(|t| (t.id.as_str(), t)).collect();

    // Build parent → children index
    let mut children_map: std::collections::HashMap<&str, Vec<&str>> =
        std::collections::HashMap::new();
    for t in &all_turns {
        if let Some(ref parent_id) = t.triggered_by_turn_id {
            children_map.entry(parent_id.as_str()).or_default().push(&t.id);
        }
    }

    // BFS from root through triggered_by links
    let mut chain = vec![];
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(root_id.as_str());

    while let Some(tid) = queue.pop_front() {
        if let Some(t) = turn_map.get(tid) {
            chain.push(TurnResponse::from(*t));
            if let Some(kids) = children_map.get(tid) {
                for kid in kids {
                    queue.push_back(kid);
                }
            }
        }
    }

    Ok(Json(chain))
}

pub async fn list_turns(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
    Query(query): Query<ListTurnsQuery>,
) -> Result<Json<Vec<TurnResponse>>, ApiError> {
    let _bearer = extract_bearer_token(&headers)?;

    let status_filter = query
        .status
        .map(|s| {
            s.parse::<TurnStatus>()
                .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid status filter"))
        })
        .transpose()?;

    let turns = state
        .workspace_store
        .list_turns(&workspace_id, status_filter)
        .await
        .map_err(ApiError::from)?;
    let responses: Vec<TurnResponse> = turns.iter().map(TurnResponse::from).collect();
    Ok(Json(responses))
}

pub async fn get_turn(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<TurnResponse>, ApiError> {
    let _bearer = extract_bearer_token(&headers)?;
    let turn = state
        .workspace_store
        .get_turn(&id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(TurnResponse::from(&turn)))
}

#[derive(Debug, serde::Deserialize)]
pub struct ResolveTurnRequest {
    pub status: String,
    pub corrected_output: Option<serde_json::Value>,
    pub reason: Option<String>,
    pub resolved_by: String,
}

pub async fn resolve_turn(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ResolveTurnRequest>,
) -> Result<Json<TurnResponse>, ApiError> {
    let _bearer = extract_bearer_token(&headers)?;

    let status: TurnStatus = body.status.parse().map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid status — expected: accepted, modified, rejected, escalated",
        )
    })?;

    if status == TurnStatus::Pending {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "cannot resolve to pending"));
    }

    // Fetch current turn to capture original_output for the feedback record
    let current = state
        .workspace_store
        .get_turn(&id)
        .await
        .map_err(ApiError::from)?;

    let feedback_action = match &status {
        TurnStatus::Accepted => FeedbackAction::Accept,
        TurnStatus::Modified => FeedbackAction::Modify,
        TurnStatus::Rejected => FeedbackAction::Reject,
        TurnStatus::Escalated => FeedbackAction::Escalate,
        TurnStatus::Pending => unreachable!(),
    };

    let feedback = Feedback {
        action: feedback_action,
        original_output: current.output_snapshot.clone(),
        corrected_output: body.corrected_output,
        reason: body.reason,
        timestamp: chrono::Utc::now(),
    };

    let resolved = state
        .workspace_store
        .resolve_turn(&id, status, Some(feedback), &body.resolved_by)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(TurnResponse::from(&resolved)))
}

#[derive(Debug, serde::Serialize)]
pub struct ReplayResponse {
    pub turn_id: String,
    pub agent_name: String,
    pub input_snapshot: serde_json::Value,
    pub original_output: serde_json::Value,
    pub replay_output: Option<serde_json::Value>,
    pub diff: Vec<DiffEntry>,
    pub original_confidence: Option<ConfidenceResponse>,
    pub replay_confidence: Option<ConfidenceResponse>,
}

#[derive(Debug, serde::Serialize)]
pub struct DiffEntry {
    pub path: String,
    pub op: String, // "add", "remove", "replace"
    pub original: Option<serde_json::Value>,
    pub replay: Option<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ReplayRequest {
    /// Optional modified input to use instead of the original
    pub modified_input: Option<serde_json::Value>,
}

pub async fn replay_turn(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ReplayRequest>,
) -> Result<Json<ReplayResponse>, ApiError> {
    let _bearer = extract_bearer_token(&headers)?;

    let replay_input = state
        .workspace_store
        .get_replay_input(&id)
        .await
        .map_err(ApiError::from)?;

    // Fetch the turn for confidence data
    let turn = state
        .workspace_store
        .get_turn(&id)
        .await
        .map_err(ApiError::from)?;

    let original_confidence = Some(ConfidenceResponse {
        score: turn.confidence.score,
        factors: turn.confidence.factors.clone(),
    });

    // Re-run the agent with __replay__ marker so mock returns a variant response
    let input_for_replay = body.modified_input.unwrap_or_else(|| replay_input.input_snapshot.clone());
    let replay_result = run_replay_agent(
        &state,
        &replay_input.agent_name,
        &input_for_replay,
    ).await;

    let (replay_output, replay_confidence, diff) = match replay_result {
        Ok((output, confidence)) => {
            let d = compute_diff("", &replay_input.original_output, &output);
            (
                Some(output),
                Some(ConfidenceResponse {
                    score: confidence.score,
                    factors: confidence.factors,
                }),
                d,
            )
        }
        Err(_) => (None, None, vec![]),
    };

    Ok(Json(ReplayResponse {
        turn_id: replay_input.turn_id,
        agent_name: replay_input.agent_name,
        input_snapshot: replay_input.input_snapshot,
        original_output: replay_input.original_output,
        replay_output,
        diff,
        original_confidence,
        replay_confidence,
    }))
}

/// Re-run an agent for replay, returning (output_json, confidence).
async fn run_replay_agent(
    state: &AppState,
    agent_name: &str,
    input_snapshot: &serde_json::Value,
) -> Result<(serde_json::Value, cliniclaw_kernel::Confidence), crate::error::ApiError> {
    // Build a minimal input that triggers the __replay__ variant in mock mode
    match agent_name {
        "ambient_doc" => {
            let input = cliniclaw_agents::AmbientDocInput {
                encounter_id: input_snapshot.get("encounter_id").and_then(|v| v.as_str()).unwrap_or("replay").to_string(),
                encounter_status: "in-progress".to_string(),
                patient_id: input_snapshot.get("patient_id").and_then(|v| v.as_str()).unwrap_or("replay").to_string(),
                practitioner_id: input_snapshot.get("practitioner_id").and_then(|v| v.as_str()).unwrap_or("replay").to_string(),
                transcript: format!("__replay__ {}", input_snapshot.get("transcript_len").and_then(|v| v.as_u64()).unwrap_or(0)),
                chief_complaint: None,
                active_medications: vec![],
                capabilities: vec!["note_generation".to_string()],
                capability_tokens: vec![],
                practitioner_role: None,
                patient_active: true,
                patient_deceased: None,
                encounter_class: None,
            };
            let output = state.ambient_agent.generate_note(&input, &state.policy_engine).await.map_err(ApiError::from)?;
            let report_json = serde_json::to_value(&output.report)?;
            Ok((report_json, output.confidence))
        }
        "order_entry" => {
            let input = cliniclaw_agents::OrderEntryInput {
                encounter_id: input_snapshot.get("encounter_id").and_then(|v| v.as_str()).unwrap_or("replay").to_string(),
                encounter_status: "in-progress".to_string(),
                patient_id: "replay".to_string(),
                practitioner_id: input_snapshot.get("practitioner_id").and_then(|v| v.as_str()).unwrap_or("replay").to_string(),
                order_text: format!("__replay__ {}", input_snapshot.get("order_text").and_then(|v| v.as_str()).unwrap_or("")),
                active_medications: vec![],
                capabilities: vec!["order_entry".to_string()],
                capability_tokens: vec![],
                practitioner_role: None,
                patient_active: true,
                patient_deceased: None,
                encounter_class: None,
            };
            let order_agent = cliniclaw_agents::OrderEntryAgent::new(state.llm.clone());
            let output = order_agent.propose_order(&input, &state.policy_engine).await.map_err(ApiError::from)?;
            let med_json = serde_json::to_value(&output.medication_request)?;
            Ok((med_json, output.confidence))
        }
        _ => {
            Err(ApiError::new(StatusCode::BAD_REQUEST, format!("replay not supported for agent: {agent_name}")))
        }
    }
}

/// Simple recursive JSON diff — compares two Values and returns diff entries.
fn compute_diff(prefix: &str, original: &serde_json::Value, replay: &serde_json::Value) -> Vec<DiffEntry> {
    let mut diffs = vec![];

    match (original, replay) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            // Keys in original
            for (key, val_a) in a {
                let path = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
                match b.get(key) {
                    Some(val_b) => {
                        diffs.extend(compute_diff(&path, val_a, val_b));
                    }
                    None => {
                        diffs.push(DiffEntry {
                            path,
                            op: "remove".to_string(),
                            original: Some(val_a.clone()),
                            replay: None,
                        });
                    }
                }
            }
            // Keys only in replay
            for (key, val_b) in b {
                if !a.contains_key(key) {
                    let path = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
                    diffs.push(DiffEntry {
                        path,
                        op: "add".to_string(),
                        original: None,
                        replay: Some(val_b.clone()),
                    });
                }
            }
        }
        (serde_json::Value::Array(a), serde_json::Value::Array(b)) => {
            if a != b {
                diffs.push(DiffEntry {
                    path: prefix.to_string(),
                    op: "replace".to_string(),
                    original: Some(original.clone()),
                    replay: Some(replay.clone()),
                });
            }
        }
        _ => {
            if original != replay {
                diffs.push(DiffEntry {
                    path: prefix.to_string(),
                    op: "replace".to_string(),
                    original: Some(original.clone()),
                    replay: Some(replay.clone()),
                });
            }
        }
    }

    diffs
}

#[derive(Debug, serde::Deserialize)]
pub struct FeedbackStatsQuery {
    pub agent_name: Option<String>,
}

pub async fn feedback_stats(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<FeedbackStatsQuery>,
) -> Result<Json<cliniclaw_kernel::FeedbackStats>, ApiError> {
    let _bearer = extract_bearer_token(&headers)?;
    let stats = state
        .workspace_store
        .get_feedback_stats(query.agent_name.as_deref())
        .await
        .map_err(ApiError::from)?;
    Ok(Json(stats))
}
