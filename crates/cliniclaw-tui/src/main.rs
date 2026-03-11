mod app;
mod event;
mod ui;

use anyhow::Result;
use crossterm::{
    event::{KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::CrosstermBackend;
use std::io::stdout;

use app::{App, RightPanel, ViewMode};
use event::{AppEvent, EventHandler};

fn main() -> Result<()> {
    let mut api_base = "http://localhost:3000".to_string();
    // Default to empty so the SSE connection receives all encounters.
    // Pass --encounter-id enc-001 to filter to a single encounter in detail mode.
    let mut encounter_id = String::new();

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--api-url" => {
                i += 1;
                if i < args.len() {
                    api_base = args[i].clone();
                }
            }
            "--encounter-id" => {
                i += 1;
                if i < args.len() {
                    encounter_id = args[i].clone();
                }
            }
            "--help" | "-h" => {
                println!("cliniclaw-tui — Terminal UI demo client for ClinicClaw");
                println!();
                println!("Usage: cliniclaw-tui [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --api-url <URL>          API base URL (default: http://localhost:3000)");
                println!("  --encounter-id <ID>      Encounter ID to filter events (default: empty = all encounters)");
                println!("                           Omit for hospital dashboard mode ([h] to toggle views).");
                println!("  -h, --help               Show this help");
                println!();
                println!("Key bindings:");
                println!("  [h]   Toggle hospital / detail view");
                println!("  [s]   Trigger full hospital simulation (POST /v1/simulate)");
                println!("  [n]   Trigger ambient note agent");
                println!("  [o]   Trigger order-entry agent");
                println!("  [p]   Trigger prior-auth agent");
                println!("  [c]   Clear event log");
                println!("  [q]   Quit");
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run(api_base, encounter_id))
}

async fn run(api_base: String, encounter_id: String) -> Result<()> {
    // Terminal setup
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    // Install panic hook that restores terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
        original_hook(info);
    }));

    let mut terminal = ratatui::Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new(encounter_id.clone());
    let mut events = EventHandler::new(&api_base, &encounter_id);
    let tx = events.tx();

    loop {
        terminal.draw(|f| {
            match app.view_mode {
                ViewMode::Detail => ui::draw(f, &mut app),
                ViewMode::Hospital => ui::draw_hospital(f, &app, f.area()),
            }
        })?;

        if let Some(ev) = events.next().await {
            match ev {
                AppEvent::Key(key) => {
                    if key.kind != crossterm::event::KeyEventKind::Press {
                        continue;
                    }
                    match (key.code, key.modifiers) {
                        (KeyCode::Char('q'), _) => {
                            app.should_quit = true;
                        }
                        // Escape: close event-detail panel if open, otherwise quit
                        (KeyCode::Esc, _) => {
                            if app.right_panel == RightPanel::EventDetail {
                                app.close_event_detail();
                            } else {
                                app.should_quit = true;
                            }
                        }
                        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => {
                            app.should_quit = true;
                        }
                        (KeyCode::Char('c'), _) => {
                            app.clear();
                        }
                        // Toggle between hospital dashboard and single-encounter detail view
                        (KeyCode::Char('h'), _) => {
                            app.view_mode = match app.view_mode {
                                ViewMode::Detail => ViewMode::Hospital,
                                ViewMode::Hospital => ViewMode::Detail,
                            };
                        }
                        // Trigger full hospital simulation (all 6 encounters)
                        (KeyCode::Char('s'), _) => {
                            app.triggering = Some("Triggering simulation...".into());
                            app.triggering_set_at = Some(std::time::Instant::now());
                            app.error_message = None;
                            event::trigger_simulate(tx.clone(), api_base.clone());
                        }
                        (KeyCode::Char('n'), _) => {
                            app.triggering = Some("Triggering note...".into());
                            app.triggering_set_at = Some(std::time::Instant::now());
                            app.error_message = None;
                            event::trigger_note(
                                tx.clone(),
                                api_base.clone(),
                                encounter_id.clone(),
                            );
                        }
                        (KeyCode::Char('o'), _) => {
                            app.triggering = Some("Triggering order...".into());
                            app.triggering_set_at = Some(std::time::Instant::now());
                            app.error_message = None;
                            event::trigger_order(
                                tx.clone(),
                                api_base.clone(),
                                encounter_id.clone(),
                            );
                        }
                        (KeyCode::Char('p'), _) => {
                            app.triggering = Some("Triggering prior auth...".into());
                            app.triggering_set_at = Some(std::time::Instant::now());
                            app.error_message = None;
                            event::trigger_prior_auth(
                                tx.clone(),
                                api_base.clone(),
                                encounter_id.clone(),
                            );
                        }
                        // Open event-detail panel for the selected event
                        (KeyCode::Enter, _) => {
                            app.open_event_detail();
                        }
                        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                            app.scroll_up();
                        }
                        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                            app.scroll_down();
                        }
                        (KeyCode::End, _) | (KeyCode::Char('G'), _) => {
                            app.jump_to_bottom();
                        }
                        _ => {}
                    }
                }
                AppEvent::Tick => {
                    // Drive time-based transitions (flash expiry, EPS decay)
                    app.on_tick();
                }
                AppEvent::AgentEvent(ae) => {
                    app.on_agent_event(ae);
                }
                AppEvent::SseConnected => {
                    app.connected = true;
                    app.error_message = None;
                }
                AppEvent::SseError(e) => {
                    app.connected = false;
                    app.error_message = Some(e);
                }
                AppEvent::TriggerResult {
                    agent,
                    success,
                    error,
                } => {
                    app.triggering = None;
                    if !success {
                        app.error_message =
                            Some(format!("{agent}: {}", error.unwrap_or_default()));
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Terminal teardown
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
