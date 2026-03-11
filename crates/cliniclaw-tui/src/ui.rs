use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{
    agent_short, event_detail, event_icon, event_label, time_delta_ms, App, PatientProgress,
    RightPanel, StageStatus, STAGES,
};
use cliniclaw_kernel::AgentEventType;

// ---------------------------------------------------------------------------
// Detail view (original single-encounter view)
// ---------------------------------------------------------------------------

pub fn draw(frame: &mut Frame, app: &mut App) {
    let has_chains = !app.chains.is_empty();
    let right_h = if has_chains || app.right_panel == RightPanel::EventDetail {
        3
    } else {
        0
    };

    let chunks = Layout::vertical([
        Constraint::Length(1),        // Header
        Constraint::Length(3),        // Pipeline
        Constraint::Min(5),           // Activity stream
        Constraint::Length(right_h),  // Chain / event-detail panel
        Constraint::Length(1),        // Help bar
    ])
    .split(frame.area());

    draw_header(frame, app, chunks[0]);
    draw_pipeline(frame, app, chunks[1]);
    draw_activity(frame, app, chunks[2]);
    if right_h > 0 {
        match app.right_panel {
            RightPanel::Chain => draw_chain(frame, app, chunks[3]),
            RightPanel::EventDetail => draw_event_detail_panel(frame, app, chunks[3]),
        }
    }
    draw_help(frame, app, chunks[4]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let conn = if app.connected {
        Span::styled("● Connected", Style::default().fg(Color::Green))
    } else {
        Span::styled("○ Disconnected", Style::default().fg(Color::Red))
    };

    let mut spans = vec![Span::styled(
        " ClinicClaw TUI ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];

    if let Some(ref msg) = app.triggering {
        spans.push(Span::styled(
            format!(" {msg} "),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if let Some(ref err) = app.error_message {
        spans.push(Span::styled(
            format!(" {err} "),
            Style::default().fg(Color::Red),
        ));
    }

    // Right-align connection status
    let used: usize = spans.iter().map(|s| s.width()).sum();
    let conn_w = conn.width();
    let padding = (area.width as usize).saturating_sub(used + conn_w);
    if padding > 0 {
        spans.push(Span::raw(" ".repeat(padding)));
    }
    spans.push(conn);

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_pipeline(frame: &mut Frame, app: &App, area: Rect) {
    let statuses = app.compute_stage_statuses();
    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::raw("  "));

    for (i, (stage, status)) in STAGES.iter().zip(statuses.iter()).enumerate() {
        let (icon, color) = match status {
            StageStatus::Waiting => ("○", Color::DarkGray),
            StageStatus::Active => ("●", Color::Blue),
            StageStatus::Completed => ("✓", Color::Green),
            StageStatus::Failed => ("✗", Color::Red),
        };

        spans.push(Span::styled(
            format!("[{icon} {stage}]"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));

        if i < STAGES.len() - 1 {
            let arrow_color = if matches!(status, StageStatus::Completed) {
                Color::Green
            } else {
                Color::DarkGray
            };
            spans.push(Span::styled("──▶", Style::default().fg(arrow_color)));
        }
    }

    let block = Block::default().borders(Borders::TOP | Borders::BOTTOM);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
}

fn draw_activity(frame: &mut Frame, app: &mut App, area: Rect) {
    let current_agent = app
        .current_run
        .first()
        .map(|e| e.agent_name.as_str())
        .unwrap_or("—");

    let title = format!(
        " Agent Activity ({}) ─── {} events ",
        current_agent,
        app.events.len()
    );

    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);

    let base_ts = app.events.first().map(|e| e.timestamp);

    let items: Vec<ListItem> = app
        .events
        .iter()
        .enumerate()
        .map(|(i, ev)| {
            let icon = event_icon(&ev.event_type);
            let label = event_label(&ev.event_type);
            let detail = event_detail(&ev.event_type);

            let icon_color = match &ev.event_type {
                AgentEventType::AgentFailed { .. } => Color::Red,
                AgentEventType::LlmCall { status, .. }
                | AgentEventType::ResponseParsing { status, .. } => {
                    match status {
                        cliniclaw_kernel::StepStatus::Started => Color::Yellow,
                        cliniclaw_kernel::StepStatus::Completed => Color::Green,
                        cliniclaw_kernel::StepStatus::Failed => Color::Red,
                    }
                }
                AgentEventType::ChainTrigger { .. } => Color::Yellow,
                _ => Color::Green,
            };

            // Show separator between agent runs
            let is_new_run =
                i > 0 && matches!(ev.event_type, AgentEventType::AgentStarted);

            let delta = base_ts
                .map(|b| format!("+{}ms", time_delta_ms(&b, &ev.timestamp)))
                .unwrap_or_default();

            let mut spans = vec![
                Span::styled(format!(" {icon} "), Style::default().fg(icon_color)),
                Span::styled(
                    format!("{:<22}", label),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ];
            if !detail.is_empty() {
                spans.push(Span::styled(
                    format!("{detail}  "),
                    Style::default().fg(Color::Gray),
                ));
            }

            // Right-pad + delta
            let used: usize = spans.iter().map(|s| s.width()).sum();
            let delta_w = delta.len();
            let avail = inner.width as usize;
            if used + delta_w < avail {
                spans.push(Span::raw(" ".repeat(avail - used - delta_w)));
            }
            spans.push(Span::styled(delta, Style::default().fg(Color::DarkGray)));

            if is_new_run {
                // Two-line item: separator + event
                ListItem::new(vec![
                    Line::from(Span::styled(
                        format!("─── {} ───", ev.agent_name),
                        Style::default().fg(Color::DarkGray),
                    )),
                    Line::from(spans),
                ])
            } else {
                ListItem::new(Line::from(spans))
            }
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_widget(block, area);
    frame.render_stateful_widget(list, inner, &mut app.list_state);
}

fn draw_chain(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Chain ")
        .borders(Borders::ALL);
    let inner = block.inner(area);

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::raw("  "));

    for (i, chain) in app.chains.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }

        let src_conf = chain
            .source_confidence
            .map(|c| format!(" {:.0}%", c * 100.0))
            .unwrap_or_default();
        let tgt_conf = chain
            .target_confidence
            .map(|c| format!(" {:.0}%", c * 100.0))
            .unwrap_or_default();

        spans.push(Span::styled(
            format!("[{}{src_conf}]", agent_short(&chain.source_agent)),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" ━━━ {} ━━━▶ ", chain.trigger_pattern),
            Style::default().fg(Color::Yellow),
        ));
        spans.push(Span::styled(
            format!("[{}{tgt_conf}]", agent_short(&chain.target_agent)),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    }

    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
}

/// Right panel shown when the user presses Enter on a selected event.
/// Renders the full event fields as key-value pairs on a single line.
fn draw_event_detail_panel(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Event Detail  [Esc] to close ")
        .borders(Borders::ALL);
    let inner = block.inner(area);

    let content = if let Some(idx) = app.list_state.selected() {
        if let Some(ev) = app.events.get(idx) {
            let label = event_label(&ev.event_type);
            let detail = event_detail(&ev.event_type);
            let ts = ev.timestamp.format("%H:%M:%S%.3f").to_string();
            format!(
                "  {} │ {} │ {} │ {} │ {}",
                ts,
                ev.agent_name,
                ev.encounter_id,
                label,
                if detail.is_empty() { "—".to_string() } else { detail },
            )
        } else {
            "  No event selected".to_string()
        }
    } else {
        "  Press ↑/↓ to select an event, then Enter to view details".to_string()
    };

    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            content,
            Style::default().fg(Color::White),
        ))),
        inner,
    );
}

fn draw_help(frame: &mut Frame, app: &App, area: Rect) {
    let key_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let sep = Span::raw("  ");

    let mut spans = vec![
        Span::raw(" "),
        Span::styled("[h]", key_style),
        Span::raw("ospital"),
        sep.clone(),
        Span::styled("[n]", key_style),
        Span::raw("ote"),
        sep.clone(),
        Span::styled("[o]", key_style),
        Span::raw("rder"),
        sep.clone(),
        Span::styled("[p]", key_style),
        Span::raw("rior-auth"),
        sep.clone(),
        Span::styled("[⏎]", key_style),
        Span::raw("detail"),
        sep.clone(),
        Span::styled("[c]", key_style),
        Span::raw("lear"),
        sep.clone(),
        Span::styled("[q]", key_style),
        Span::raw("uit"),
    ];

    // Right-align encounter ID
    let used: usize = spans.iter().map(|s| s.width()).sum();
    let enc_w = app.encounter_id.len();
    let pad = (area.width as usize).saturating_sub(used + enc_w + 1);
    if pad > 0 {
        spans.push(Span::raw(" ".repeat(pad)));
    }
    spans.push(Span::styled(
        app.encounter_id.clone(),
        Style::default().fg(Color::DarkGray),
    ));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ---------------------------------------------------------------------------
// Hospital dashboard view
// ---------------------------------------------------------------------------

/// Top-level hospital dashboard — call from main when `view_mode == Hospital`.
pub fn draw_hospital(f: &mut Frame, app: &App, area: Rect) {
    let layout = Layout::vertical([
        Constraint::Length(1), // header bar
        Constraint::Length(3), // metrics bar
        Constraint::Min(5),    // main content
        Constraint::Length(1), // help bar
    ])
    .split(area);

    draw_hospital_header(f, app, layout[0]);
    draw_metrics_bar(f, app, layout[1]);

    // Split content: narrow patient sidebar | wider activity feed
    let content = Layout::horizontal([
        Constraint::Length(18),
        Constraint::Min(30),
    ])
    .split(layout[2]);

    draw_patient_sidebar(f, app, content[0]);
    draw_hospital_activity(f, app, content[1]);
    draw_hospital_help(f, app, layout[3]);
}

fn draw_hospital_header(f: &mut Frame, app: &App, area: Rect) {
    let title = Span::styled(
        " ClinicClaw Hospital Simulation ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    // Show flash message if present, otherwise connection + agent count
    let right_span = if let Some(ref msg) = app.triggering {
        Span::styled(
            format!(" {msg} "),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        let conn_str = if app.connected {
            format!("● Connected  {} active", app.metrics.active_agents)
        } else {
            "○ Disconnected".to_string()
        };
        let conn_color = if app.connected { Color::Green } else { Color::Red };
        Span::styled(conn_str, Style::default().fg(conn_color))
    };

    let title_w = title.width();
    let right_w = right_span.width();
    let pad = (area.width as usize).saturating_sub(title_w + right_w);

    let spans = vec![title, Span::raw(" ".repeat(pad)), right_span];
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Live metrics bar rendered just below the hospital header.
fn draw_metrics_bar(f: &mut Frame, app: &App, area: Rect) {
    let m = &app.metrics;

    let block = Block::default()
        .title(" METRICS ")
        .borders(Borders::ALL);
    let inner = block.inner(area);

    let label_style = Style::default().fg(Color::DarkGray);
    let val_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let allow_style = Style::default().fg(Color::Green).add_modifier(Modifier::BOLD);
    let deny_style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
    let req_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let sep = Span::raw("   ");

    let spans = vec![
        Span::raw(" "),
        Span::styled("Events: ", label_style),
        Span::styled(format!("{}", m.total_events), val_style),
        sep.clone(),
        Span::styled("EPS: ", label_style),
        Span::styled(format!("{:.1}", m.events_per_second), val_style),
        sep.clone(),
        Span::styled("Active agents: ", label_style),
        Span::styled(format!("{}", m.active_agents), val_style),
        sep.clone(),
        Span::styled("Policy — ", label_style),
        Span::styled("allow: ", label_style),
        Span::styled(format!("{}", m.policy_allow), allow_style),
        Span::raw("  "),
        Span::styled("deny: ", label_style),
        Span::styled(format!("{}", m.policy_deny), deny_style),
        Span::raw("  "),
        Span::styled("require_approval: ", label_style),
        Span::styled(format!("{}", m.policy_require_approval), req_style),
    ];

    f.render_widget(block, area);
    f.render_widget(Paragraph::new(Line::from(spans)), inner);
}

fn draw_patient_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title("PATIENTS")
        .borders(Borders::ALL);
    let inner = block.inner(area);

    let statuses = app.patient_statuses();

    let total_turns: usize = statuses.iter().map(|p| p.turn_count).sum();
    let total_agents: usize = statuses.iter().map(|p| p.agent_count).sum();

    // Each patient occupies one line; summary on the last line
    let mut items: Vec<ListItem> = statuses
        .iter()
        .map(|p| {
            let (icon, color) = match p.status {
                PatientProgress::Active => ("●", Color::Yellow),
                PatientProgress::InProgress => ("✓", Color::Green),
                PatientProgress::Waiting => ("○", Color::DarkGray),
            };

            // Show last-agent abbreviation when in-progress/active
            let last_abbrev = p
                .last_agent
                .as_deref()
                .map(agent_short)
                .unwrap_or("  ");

            let line = Line::from(vec![
                Span::styled(format!(" {icon} "), Style::default().fg(color)),
                Span::styled(
                    format!("{:<9}", p.name),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}", last_abbrev),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    // Summary row at the bottom
    let summary = format!(" {} pts/{} t/{} a", statuses.len(), total_turns, total_agents);
    items.push(ListItem::new(Line::from(Span::styled(
        summary,
        Style::default().fg(Color::DarkGray),
    ))));

    f.render_widget(block, area);
    f.render_widget(List::new(items), inner);
}

fn draw_hospital_activity(f: &mut Frame, app: &App, area: Rect) {
    let title = format!(" LIVE ACTIVITY ─── {} events ", app.events.len());
    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);

    let base_ts = app.events.first().map(|e| e.timestamp);

    let items: Vec<ListItem> = app
        .events
        .iter()
        .enumerate()
        .map(|(i, ev)| {
            let icon = event_icon(&ev.event_type);
            let label = event_label(&ev.event_type);
            let abbrev = agent_short(&ev.agent_name);

            // Look up patient name from the dynamic roster
            let patient = app
                .patient_roster
                .get(&ev.encounter_id)
                .map(|(name, _)| name.as_str())
                .unwrap_or(&ev.encounter_id);

            let icon_color = match &ev.event_type {
                AgentEventType::AgentFailed { .. } => Color::Red,
                AgentEventType::LlmCall { status, .. }
                | AgentEventType::ResponseParsing { status, .. } => match status {
                    cliniclaw_kernel::StepStatus::Started => Color::Yellow,
                    cliniclaw_kernel::StepStatus::Completed => Color::Green,
                    cliniclaw_kernel::StepStatus::Failed => Color::Red,
                },
                AgentEventType::ChainTrigger { .. } => Color::Yellow,
                _ => Color::Green,
            };

            // Show agent separator when a new agent run begins
            let is_new_run =
                i > 0 && matches!(ev.event_type, AgentEventType::AgentStarted);

            let time_str = base_ts
                .map(|b| format!("+{}ms", time_delta_ms(&b, &ev.timestamp)))
                .unwrap_or_default();

            // Format: "{time}  [{abbrev}] {patient:<9}  {icon} {label}"
            let mut spans = vec![
                Span::styled(
                    format!("{:<8}", time_str),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("[{abbrev}]"),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{:<9}", patient),
                    Style::default().fg(Color::White),
                ),
                Span::raw(" "),
                Span::styled(format!("{icon} "), Style::default().fg(icon_color)),
                Span::styled(label, Style::default().add_modifier(Modifier::BOLD)),
            ];

            // Right-pad to fill width (avoids ragged right edge in scrollable list)
            let used: usize = spans.iter().map(|s| s.width()).sum();
            let avail = inner.width as usize;
            if used < avail {
                spans.push(Span::raw(" ".repeat(avail.saturating_sub(used))));
            }

            if is_new_run {
                ListItem::new(vec![
                    Line::from(Span::styled(
                        format!("─── {} / {} ───", patient, ev.agent_name),
                        Style::default().fg(Color::DarkGray),
                    )),
                    Line::from(spans),
                ])
            } else {
                ListItem::new(Line::from(spans))
            }
        })
        .collect();

    // Auto-scroll: show the last page of events
    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    f.render_widget(block, area);

    // Use a stateful render so the list scrolls to the bottom automatically.
    let mut state = ratatui::widgets::ListState::default();
    if !app.events.is_empty() && app.auto_scroll {
        state.select(Some(app.events.len() - 1));
    }
    f.render_stateful_widget(list, inner, &mut state);
}

fn draw_hospital_help(f: &mut Frame, app: &App, area: Rect) {
    let key_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let sep = Span::raw("  ");

    let mut spans = vec![
        Span::raw(" "),
        Span::styled("[s]", key_style),
        Span::raw("imulate"),
        sep.clone(),
        Span::styled("[h]", key_style),
        Span::raw("ospital/detail"),
        sep.clone(),
        Span::styled("[n]", key_style),
        Span::raw("ote"),
        sep.clone(),
        Span::styled("[o]", key_style),
        Span::raw("rder"),
        sep.clone(),
        Span::styled("[p]", key_style),
        Span::raw("rior-auth"),
        sep.clone(),
        Span::styled("[c]", key_style),
        Span::raw("lear"),
        sep.clone(),
        Span::styled("[q]", key_style),
        Span::raw("uit"),
    ];

    // Right-align error message if present
    if let Some(ref err) = app.error_message {
        let used: usize = spans.iter().map(|s| s.width()).sum();
        let err_w = err.len() + 1;
        let pad = (area.width as usize).saturating_sub(used + err_w);
        if pad > 0 {
            spans.push(Span::raw(" ".repeat(pad)));
        }
        spans.push(Span::styled(err.clone(), Style::default().fg(Color::Red)));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
