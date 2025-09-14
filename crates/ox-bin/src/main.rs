//! Oxidized entrypoint – Phase 0 skeleton.

use anyhow::Result;
use core_actions::Action;
use core_actions::ActionObserver; // trait (currently unused in main but stored for future use)
use core_actions::dispatcher::dispatch;
use core_events::{CommandEvent, Event, InputEvent, KeyEvent};
// NOTE: `EVENT_CHANNEL_CAP` lives in `core-events` (currently unused while channel is unbounded).
// When introducing additional async producers, migrate to `mpsc::channel(EVENT_CHANNEL_CAP)` and
// implement documented backpressure policy (Refactor R1 Step 10).
// use core_events::EVENT_CHANNEL_CAP; // (future bounded channel capacity activation point)
use core_render::scheduler::RenderScheduler;
use core_render::status::{StatusContext, build_status};
use core_render::viewport::Viewport;
use core_render::{Frame, Renderer};
use core_state::EditorState;
use core_state::Mode;
use core_terminal::{CrosstermBackend, TerminalBackend};
use core_text::Buffer;
use core_text::grapheme;
use tokio::sync::mpsc;
use tracing::{error, info};

// RenderScheduler moved to core-render::scheduler (Refactor R1 Step 4)

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
    let mut state = EditorState::new(buffer);
    // Async unbounded channel (single consumer main loop). Input thread forwards blocking
    // crossterm events. Future bounded migration: swap to `mpsc::channel(EVENT_CHANNEL_CAP)` when
    // the first additional async producer (config watcher, timers, LSP, plugin host) lands.
    let (tx, mut rx) = mpsc::unbounded_channel::<Event>();
    let _input_handle = core_input::spawn_input_thread(tx.clone());

    // Command line now stored within EditorState (Refactor R1 Step 2)

    // Sticky visual column for vertical motions (None until first j/k).
    let mut sticky_visual_col: Option<usize> = None;

    // Initial render so the user sees content before pressing a key.
    if let Err(e) = render(&state) {
        error!(?e, "initial render error");
    }

    let render_span = tracing::info_span!("event_loop");
    let _enter_loop = render_span.enter();
    let mut scheduler = RenderScheduler::new();
    // Refactor R1 Step 8: prepare empty observer list (macro recorder, analytics to be added later).
    let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
    while let Some(event) = rx.recv().await {
        // NOTE: No polling – loop wakes only on incoming events from channel.
        match event {
            Event::Input(InputEvent::CtrlC) => {
                info!("shutdown");
                break;
            }
            Event::Input(InputEvent::Key(k)) => {
                // Single unified path: every key translated (breadth-first simplicity)
                if let Some(act) =
                    translate_key_wrapper(state.mode, state.command_line.buffer(), &k)
                {
                    let dr = dispatch(act, &mut state, &mut sticky_visual_col, &observers);
                    if dr.dirty {
                        scheduler.mark_dirty();
                    }
                    if dr.quit {
                        break;
                    }
                }
            }
            Event::Input(InputEvent::Resize(_, _)) => { /* trigger redraw below */ }
            Event::RenderRequested => {}
            Event::Command(CommandEvent::Quit) => {
                break;
            }
            Event::Shutdown => {
                break;
            }
        }
        // Ask scheduler if a redraw is needed this cycle.
        if scheduler.consume_dirty()
            && let Err(e) = render(&state)
        {
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

    // Viewport: reserve one line for status if possible
    let text_height = if h > 0 { h - 1 } else { 0 };
    let mut viewport = Viewport::new(0, text_height as usize);
    viewport.clamp_cursor_into_view(state.position.line); // no-op now
    let buf = state.active_buffer();
    let start = viewport.first_line;
    let end = (start + viewport.height).min(buf.line_count());
    for (screen_y, line_idx) in (start..end).enumerate() {
        if (screen_y as u16) >= text_height {
            break;
        }
        if let Some(line) = buf.line(line_idx) {
            for (x, ch) in line.chars().enumerate() {
                if (x as u16) < w {
                    frame.set(x as u16, screen_y as u16, ch);
                }
            }
        }
    }
    // Mode / status line (bottom) via formatter module
    if h > 0 {
        let y = h - 1;
        let buf = state.active_buffer();
        let line_content = buf.line(state.position.line).unwrap_or_default();
        let content_trim = if line_content.ends_with('\n') {
            &line_content[..line_content.len() - 1]
        } else {
            &line_content
        };
        let col = grapheme::visual_col(content_trim, state.position.byte);
        let status = build_status(&StatusContext {
            mode: state.mode,
            line: state.position.line,
            col,
            command_active: state.command_line.is_active(),
            command_buffer: state.command_line.buffer(),
        });
        for (i, ch) in status.chars().enumerate() {
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

fn translate_key_wrapper(mode: Mode, pending_command: &str, key: &KeyEvent) -> Option<Action> {
    core_actions::translate_key(mode, pending_command, key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_actions::{EditKind, ModeChange};
    use core_text::Buffer;

    fn mk_state(initial: &str) -> EditorState {
        let buf = Buffer::from_str("test", initial).unwrap();
        EditorState::new(buf)
    }

    #[test]
    fn insert_newline_coalescing_boundary() {
        let mut state = mk_state("");
        let mut sticky = None;
        // Enter insert
        let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut state,
            &mut sticky,
            &observers,
        );
        // Insert 'a'
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("a".to_string())),
            &mut state,
            &mut sticky,
            &observers,
        );
        assert_eq!(state.active_buffer().line(0).unwrap(), "a");
        assert_eq!(state.undo_stack.len(), 1, "expected first snapshot");
        // Newline
        dispatch(
            Action::Edit(EditKind::InsertNewline),
            &mut state,
            &mut sticky,
            &observers,
        );
        assert_eq!(state.active_buffer().line_count(), 2);
        assert_eq!(state.active_buffer().line(0).unwrap(), "a\n");
        assert_eq!(state.position.line, 1);
        assert_eq!(state.position.byte, 0);
        // Insert 'b' (new run -> new snapshot)
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("b".to_string())),
            &mut state,
            &mut sticky,
            &observers,
        );
        assert_eq!(state.active_buffer().line(1).unwrap(), "b");
        assert_eq!(
            state.undo_stack.len(),
            2,
            "expected second snapshot after new run"
        );
    }

    #[test]
    fn backspace_stays_within_run_dispatch() {
        let mut state = mk_state("");
        let mut sticky = None;
        let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut state,
            &mut sticky,
            &observers,
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("a".to_string())),
            &mut state,
            &mut sticky,
            &observers,
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("b".to_string())),
            &mut state,
            &mut sticky,
            &observers,
        );
        assert_eq!(state.active_buffer().line(0).unwrap(), "ab");
        assert_eq!(state.undo_stack.len(), 1, "still single run snapshot");
        // Backspace
        dispatch(
            Action::Edit(EditKind::Backspace),
            &mut state,
            &mut sticky,
            &observers,
        );
        assert_eq!(state.active_buffer().line(0).unwrap(), "a");
        assert_eq!(
            state.undo_stack.len(),
            1,
            "backspace should not create new snapshot"
        );
        // Leave insert -> ends run
        dispatch(
            Action::ModeChange(ModeChange::LeaveInsert),
            &mut state,
            &mut sticky,
            &observers,
        );
        // Undo should revert entire sequence (buffer empty)
        assert!(dispatch(Action::Undo, &mut state, &mut sticky, &observers).dirty);
        assert_eq!(state.active_buffer().line(0).unwrap(), "");
    }

    #[test]
    fn normal_mode_delete_under_single() {
        let mut state = mk_state("abc");
        let mut sticky = None;
        // Delete 'a'
        let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
        dispatch(
            Action::Edit(EditKind::DeleteUnder),
            &mut state,
            &mut sticky,
            &observers,
        );
        assert_eq!(state.active_buffer().line(0).unwrap(), "bc");
        assert_eq!(state.undo_stack.len(), 1, "snapshot pushed for delete");
        // Undo
        assert!(dispatch(Action::Undo, &mut state, &mut sticky, &observers).dirty);
        assert_eq!(state.active_buffer().line(0).unwrap(), "abc");
    }

    #[test]
    fn normal_mode_delete_under_multiple_and_undo() {
        let mut state = mk_state("abcd");
        let mut sticky = None;
        // Delete 'a'
        let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
        dispatch(
            Action::Edit(EditKind::DeleteUnder),
            &mut state,
            &mut sticky,
            &observers,
        );
        // Delete 'b' (originally 'c', now at index 0 after first delete)
        dispatch(
            Action::Edit(EditKind::DeleteUnder),
            &mut state,
            &mut sticky,
            &observers,
        );
        assert_eq!(state.active_buffer().line(0).unwrap(), "cd");
        assert_eq!(state.undo_stack.len(), 2, "two discrete snapshots");
        // Undo last -> should restore to "bcd" (?) Actually sequence: start abcd -> after first delete bcd -> after second delete cd. Undo should return to bcd.
        assert!(dispatch(Action::Undo, &mut state, &mut sticky, &observers).dirty);
        assert_eq!(state.active_buffer().line(0).unwrap(), "bcd");
        // Undo again -> original
        assert!(dispatch(Action::Undo, &mut state, &mut sticky, &observers).dirty);
        assert_eq!(state.active_buffer().line(0).unwrap(), "abcd");
    }
}
