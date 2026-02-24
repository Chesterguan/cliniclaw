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

use app::App;
use event::{AppEvent, EventHandler};

fn main() -> Result<()> {
    let mut api_base = "http://localhost:3000".to_string();
    let mut encounter_id = "enc-001".to_string();

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
                println!("  --encounter-id <ID>      Encounter ID (default: enc-001)");
                println!("  -h, --help               Show this help");
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
        terminal.draw(|f| ui::draw(f, &mut app))?;

        if let Some(ev) = events.next().await {
            match ev {
                AppEvent::Key(key) => {
                    if key.kind != crossterm::event::KeyEventKind::Press {
                        continue;
                    }
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            app.should_quit = true;
                        }
                        KeyCode::Char('c')
                            if key.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            app.should_quit = true;
                        }
                        KeyCode::Char('n') => {
                            app.triggering = Some("Triggering note...".into());
                            app.error_message = None;
                            event::trigger_note(
                                tx.clone(),
                                api_base.clone(),
                                encounter_id.clone(),
                            );
                        }
                        KeyCode::Char('o') => {
                            app.triggering = Some("Triggering order...".into());
                            app.error_message = None;
                            event::trigger_order(
                                tx.clone(),
                                api_base.clone(),
                                encounter_id.clone(),
                            );
                        }
                        KeyCode::Char('p') => {
                            app.triggering = Some("Triggering prior auth...".into());
                            app.error_message = None;
                            event::trigger_prior_auth(
                                tx.clone(),
                                api_base.clone(),
                                encounter_id.clone(),
                            );
                        }
                        KeyCode::Char('c') => {
                            app.clear();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.scroll_up();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.scroll_down();
                        }
                        KeyCode::End | KeyCode::Char('G') => {
                            app.jump_to_bottom();
                        }
                        _ => {}
                    }
                }
                AppEvent::Tick => {
                    // Redraw handled by loop
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
