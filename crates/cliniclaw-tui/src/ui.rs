use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{
    agent_short, event_detail, event_icon, event_label, time_delta_ms, App, StageStatus, STAGES,
};
use cliniclaw_kernel::AgentEventType;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let has_chains = !app.chains.is_empty();
    let chain_h = if has_chains { 3 } else { 0 };

    let chunks = Layout::vertical([
        Constraint::Length(1),      // Header
        Constraint::Length(3),      // Pipeline
        Constraint::Min(5),         // Activity stream
        Constraint::Length(chain_h), // Chain panel
        Constraint::Length(1),      // Help bar
    ])
    .split(frame.area());

    draw_header(frame, app, chunks[0]);
    draw_pipeline(frame, app, chunks[1]);
    draw_activity(frame, app, chunks[2]);
    if has_chains {
        draw_chain(frame, app, chunks[3]);
    }
    draw_help(frame, app, chunks[4]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let conn = if app.connected {
        Span::styled("● Connected", Style::default().fg(Color::Green))
    } else {
        Span::styled("○ Disconnected", Style::default().fg(Color::Red))
    };

    let mut spans = vec![
        Span::styled(
            " ClinicClaw TUI ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    if let Some(ref msg) = app.triggering {
        spans.push(Span::styled(
            format!(" {msg} "),
            Style::default().fg(Color::Yellow),
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
            let is_new_run = i > 0
                && matches!(ev.event_type, AgentEventType::AgentStarted);

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

fn draw_help(frame: &mut Frame, app: &App, area: Rect) {
    let key_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let sep = Span::raw("  ");

    let mut spans = vec![
        Span::raw(" "),
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
