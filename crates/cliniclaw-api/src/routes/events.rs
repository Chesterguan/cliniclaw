use std::convert::Infallible;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::stream::Stream;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::state::AppState;

#[derive(Debug, serde::Deserialize)]
pub struct EventStreamQuery {
    /// Filter events to a specific encounter. If omitted, receives all events.
    pub encounter_id: Option<String>,
}

/// SSE endpoint: `GET /v1/events?encounter_id=xxx`
///
/// Streams real-time agent events to the frontend. Each event is a JSON
/// AgentEvent serialized as an SSE `data:` line. The connection stays open
/// indefinitely with keep-alive pings every 15 seconds.
pub async fn event_stream(
    State(state): State<Arc<AppState>>,
    Query(query): Query<EventStreamQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.event_tx.subscribe();
    let encounter_filter = query.encounter_id;

    let stream = BroadcastStream::new(rx)
        .filter_map(move |result| {
            match result {
                Ok(event) => {
                    // Filter by encounter_id if specified
                    if let Some(ref filter) = encounter_filter {
                        if &event.encounter_id != filter {
                            return None;
                        }
                    }
                    let json = serde_json::to_string(&event).ok()?;
                    Some(Ok(Event::default().data(json)))
                }
                // Lagged — subscriber couldn't keep up; skip silently
                Err(_) => None,
            }
        });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
