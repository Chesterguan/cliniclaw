use cliniclaw_kernel::AgentEvent;
use crossterm::event::{Event as CtEvent, EventStream, KeyEvent};
use futures::StreamExt;
use reqwest_eventsource::{Event as SseEvent, EventSource};
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    AgentEvent(AgentEvent),
    SseConnected,
    SseError(String),
    TriggerResult {
        agent: String,
        success: bool,
        error: Option<String>,
    },
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
    _tx: mpsc::UnboundedSender<AppEvent>,
}

impl EventHandler {
    pub fn new(api_base: &str, encounter_id: &str) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        // Crossterm key events
        let key_tx = tx.clone();
        tokio::spawn(async move {
            let mut reader = EventStream::new();
            loop {
                if let Some(Ok(CtEvent::Key(key))) = reader.next().await {
                    if key_tx.send(AppEvent::Key(key)).is_err() {
                        break;
                    }
                }
            }
        });

        // SSE stream
        let sse_tx = tx.clone();
        let sse_url = if encounter_id.is_empty() {
            format!("{api_base}/v1/events")
        } else {
            format!("{api_base}/v1/events?encounter_id={encounter_id}")
        };
        tokio::spawn(async move {
            loop {
                let mut es = EventSource::get(&sse_url);
                while let Some(event) = es.next().await {
                    match event {
                        Ok(SseEvent::Open) => {
                            let _ = sse_tx.send(AppEvent::SseConnected);
                        }
                        Ok(SseEvent::Message(msg)) => {
                            match serde_json::from_str::<AgentEvent>(&msg.data) {
                                Ok(ae) => {
                                    let _ = sse_tx.send(AppEvent::AgentEvent(ae));
                                }
                                Err(e) => {
                                    let _ = sse_tx.send(AppEvent::SseError(format!(
                                        "parse: {e}"
                                    )));
                                }
                            }
                        }
                        Err(e) => {
                            let _ =
                                sse_tx.send(AppEvent::SseError(format!("{e}")));
                            es.close();
                            break;
                        }
                    }
                }
                // Reconnect after 2s
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        });

        // Tick timer
        let tick_tx = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(250));
            loop {
                interval.tick().await;
                if tick_tx.send(AppEvent::Tick).is_err() {
                    break;
                }
            }
        });

        Self { rx, _tx: tx }
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }

    pub fn tx(&self) -> mpsc::UnboundedSender<AppEvent> {
        self._tx.clone()
    }
}

pub fn trigger_note(
    tx: mpsc::UnboundedSender<AppEvent>,
    api_base: String,
    encounter_id: String,
) {
    tokio::spawn(async move {
        let url = format!("{api_base}/v1/encounter/{encounter_id}/note");
        let body = serde_json::json!({
            "practitioner_id": "pract-001",
            "transcript": "Patient is a 58-year-old male presenting with recurring headaches over the past two weeks. Currently taking lisinopril 10mg daily for hypertension. Blood pressure today 138/88. Reports headaches are worse in the morning, rates pain 6/10. No visual changes, no nausea. Has been compliant with medications.",
            "chief_complaint": "Recurring headaches",
            "active_medications": ["lisinopril 10mg daily"],
            "practitioner_role": "physician"
        });
        let result = reqwest::Client::new()
            .post(&url)
            .header("Authorization", "Bearer demo-token")
            .json(&body)
            .send()
            .await;
        match result {
            Ok(resp) if resp.status().is_success() => {
                let _ = tx.send(AppEvent::TriggerResult {
                    agent: "ambient_doc".into(),
                    success: true,
                    error: None,
                });
            }
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                let _ = tx.send(AppEvent::TriggerResult {
                    agent: "ambient_doc".into(),
                    success: false,
                    error: Some(format!("{status}: {text}")),
                });
            }
            Err(e) => {
                let _ = tx.send(AppEvent::TriggerResult {
                    agent: "ambient_doc".into(),
                    success: false,
                    error: Some(e.to_string()),
                });
            }
        }
    });
}

pub fn trigger_order(
    tx: mpsc::UnboundedSender<AppEvent>,
    api_base: String,
    encounter_id: String,
) {
    tokio::spawn(async move {
        let url = format!("{api_base}/v1/encounter/{encounter_id}/orders");
        let body = serde_json::json!({
            "practitioner_id": "pract-001",
            "order_text": "Start metformin 500mg BID for type 2 diabetes",
            "active_medications": ["lisinopril 10mg daily"],
            "practitioner_role": "physician"
        });
        let result = reqwest::Client::new()
            .post(&url)
            .header("Authorization", "Bearer demo-token")
            .json(&body)
            .send()
            .await;
        match result {
            Ok(resp) if resp.status().is_success() => {
                let _ = tx.send(AppEvent::TriggerResult {
                    agent: "order_entry".into(),
                    success: true,
                    error: None,
                });
            }
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                let _ = tx.send(AppEvent::TriggerResult {
                    agent: "order_entry".into(),
                    success: false,
                    error: Some(format!("{status}: {text}")),
                });
            }
            Err(e) => {
                let _ = tx.send(AppEvent::TriggerResult {
                    agent: "order_entry".into(),
                    success: false,
                    error: Some(e.to_string()),
                });
            }
        }
    });
}

pub fn trigger_prior_auth(
    tx: mpsc::UnboundedSender<AppEvent>,
    api_base: String,
    encounter_id: String,
) {
    tokio::spawn(async move {
        let url = format!("{api_base}/v1/encounter/{encounter_id}/prior-auth");
        let body = serde_json::json!({
            "practitioner_id": "pract-001",
            "service_request_id": "sr-001",
            "service_description": "Bilateral total knee replacement (arthroplasty)",
            "diagnosis_codes": ["M17.11", "M17.12"],
            "cpt_codes": ["27447"],
            "clinical_notes": "Patient has severe bilateral osteoarthritis of both knees, failed conservative management including 6 months of physical therapy, NSAIDs, and corticosteroid injections. BMI 28. No contraindications to surgery.",
            "practitioner_role": "physician"
        });
        let result = reqwest::Client::new()
            .post(&url)
            .header("Authorization", "Bearer demo-token")
            .json(&body)
            .send()
            .await;
        match result {
            Ok(resp) if resp.status().is_success() => {
                let _ = tx.send(AppEvent::TriggerResult {
                    agent: "prior_auth".into(),
                    success: true,
                    error: None,
                });
            }
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                let _ = tx.send(AppEvent::TriggerResult {
                    agent: "prior_auth".into(),
                    success: false,
                    error: Some(format!("{status}: {text}")),
                });
            }
            Err(e) => {
                let _ = tx.send(AppEvent::TriggerResult {
                    agent: "prior_auth".into(),
                    success: false,
                    error: Some(e.to_string()),
                });
            }
        }
    });
}
