//! Oxidized entrypoint – Phase 0 skeleton.

use anyhow::Result;
use core_events::{CommandEvent, Event, InputEvent, KeyCode};
use core_render::{Frame, Renderer};
use core_state::EditorState;
use core_terminal::{CrosstermBackend, TerminalBackend};
use core_text::Buffer;
use std::sync::mpsc;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Set up file logging to oxidized.log (append mode, non-blocking).
    let log_dir = std::path::Path::new(".");
    let log_path = log_dir.join("oxidized.log");
    if log_path.exists() {
        let _ = std::fs::remove_file(&log_path);
    }
    let file_appender = tracing_appender::rolling::never(log_dir, "oxidized.log");
    let (nb_writer, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(nb_writer)
        .init();

    info!("Oxidized Phase 0 starting");

    // Install a panic hook to ensure we log unexpected panics before the
    // terminal is restored by the backend's Drop impl.
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(?info, "panic");
        // Call the default (prints to stderr) so the user still sees it.
        default_panic(info);
    }));

    let mut term = CrosstermBackend::new();
    term.set_title("Oxidized")?; // set title before entering alt screen
    // Use RAII guard so any early return/panic restores the terminal.
    let _term_guard = term.enter_guard()?;

    let buffer = Buffer::from_str(
        "welcome",
        "Welcome to Oxidized (Phase 0)\nPress :q to quit.",
    )?;
    let state = EditorState::new(buffer);
    let (tx, rx) = mpsc::channel::<Event>();
    let _input_handle = core_input::spawn_input_thread(tx.clone());

    // Simple command mode detection
    let mut pending_command = String::new();

    let render_span = tracing::info_span!("event_loop");
    let _enter_loop = render_span.enter();
    while let Ok(event) = rx.recv() {
        match event {
            Event::Input(InputEvent::CtrlC) => {
                info!("shutdown");
                break;
            }
            Event::Input(InputEvent::Key(k)) => match k.code {
                KeyCode::Colon => {
                    pending_command.clear();
                    pending_command.push(':');
                }
                KeyCode::Char(c) => {
                    if pending_command.starts_with(':') {
                        pending_command.push(c);
                    }
                }
                KeyCode::Enter => {
                    if pending_command == ":q" {
                        break;
                    }
                    pending_command.clear();
                }
                KeyCode::Esc => {
                    pending_command.clear();
                }
                _ => {}
            },
            Event::Input(InputEvent::Resize(_, _)) => { /* trigger redraw below */ }
            Event::RenderRequested => {}
            Event::Command(CommandEvent::Quit) => {
                break;
            }
            Event::Shutdown => {
                break;
            }
        }

        // After handling any event, render.
        if let Err(e) = render(&state) {
            error!(?e, "render error");
        }
    }
    // Guard drop will restore terminal.
    info!("Oxidized Phase 0 exiting");
    Ok(())
}

fn render(state: &EditorState) -> Result<()> {
    use crossterm::terminal::size;
    let (w, h) = size()?;
    let mut frame = Frame::new(w, h);

    let buf = state.active_buffer();
    for (i, line_idx) in (0..buf.line_count()).enumerate() {
        if (i as u16) >= h {
            break;
        }
        if let Some(line) = buf.line(line_idx) {
            for (x, ch) in line.chars().enumerate() {
                if (x as u16) < w {
                    frame.set(x as u16, i as u16, ch);
                }
            }
        }
    }
    // Mode indicator (bottom line)
    if h > 0 {
        let mode_str = "-- NORMAL --";
        let y = h - 1; // last line
        for (i, ch) in mode_str.chars().enumerate() {
            if (i as u16) < w {
                frame.set(i as u16, y, ch);
            }
        }
    }
    let span = tracing::info_span!("render_cycle");
    let _e = span.enter();
    Renderer::render(&frame)?;
    Ok(())
}
