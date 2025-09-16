//! Oxidized entrypoint – Phase 0 skeleton.

use anyhow::Result;
use clap::Parser;
use core_actions::Action;
use core_actions::ActionObserver; // trait (currently unused in main but stored for future use)
use core_actions::dispatcher::dispatch;
use core_config::load_from;
use core_events::{CommandEvent, EVENT_CHANNEL_CAP, Event, InputEvent, KeyEvent};
use core_render::scheduler::{RenderDelta, RenderScheduler};
// Frame kept for tests still referencing type
use core_render::render_engine::RenderEngine;
use core_state::Mode;
use core_state::{EditorState, normalize_line_endings};
use core_terminal::{CrosstermBackend, TerminalBackend};
use core_text::Buffer;
use tokio::sync::mpsc;
use tracing::{error, info};

// RenderScheduler moved to core-render::scheduler (Refactor R1 Step 4)

/// CLI arguments (Phase 2 Step 2: optional file path for initial open)
#[derive(Parser, Debug)]
#[command(name = "oxidized", version, about = "Oxidized editor")] // minimal metadata
struct Args {
    /// Optional path to open at startup (UTF-8 text). If omitted a welcome buffer is used.
    pub path: Option<std::path::PathBuf>,
    /// Optional configuration file path (overrides discovery of `oxidized.toml`).
    #[arg(long = "config")]
    pub config: Option<std::path::PathBuf>,
}

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

    let args = Args::parse();

    // Base buffer: either from file or welcome text.
    let (buffer, file_name, norm_meta) = if let Some(path) = args.path {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let norm = normalize_line_endings(&content);
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("file");
                (
                    Buffer::from_str(name, &norm.normalized)?,
                    Some(path),
                    Some(norm),
                )
            }
            Err(e) => {
                error!(?e, "file_open_error");
                let buf = Buffer::from_str(
                    "welcome",
                    "Welcome to ⚙️ Oxidized (Phase 2)\n(File open failed; starting empty)\nPress :q to quit.",
                )?;
                (buf, None, None)
            }
        }
    } else {
        (
            Buffer::from_str(
                "welcome",
                "Welcome to ⚙️ Oxidized (Phase 2)\nPress :q to quit.",
            )?,
            None,
            None,
        )
    };
    let mut state = EditorState::new(buffer);
    state.file_name = file_name;
    if let Some(n) = norm_meta {
        state.original_line_ending = n.original;
        state.had_trailing_newline = n.had_trailing_newline;
        if n.mixed {
            tracing::warn!("mixed_line_endings_detected_startup");
        }
    }
    state.dirty = false; // explicit for clarity (new buffer always clean at load)
    if state.file_name.is_none() {
        // Detect prior error case via welcome buffer content heuristic; ephemeral message for visibility.
        if state.active_buffer().name == "welcome"
            && state
                .active_buffer()
                .line(0)
                .unwrap_or_default()
                .contains("File open failed")
        {
            state.set_ephemeral("Open failed", std::time::Duration::from_secs(3));
        }
    }
    // Load configuration (Phase 2 Step 14). We parse early so margin can influence initial scroll decisions.
    let mut config = load_from(args.config.clone())?; // args consumed earlier for path
    if let Ok((_w, h)) = crossterm::terminal::size() {
        // initial viewport height
        config.apply_viewport_height(h.saturating_sub(1)); // text rows (exclude status)
    }
    // Store effective margin inside state (temporary field addition in Phase 2 Step 14).
    state.config_vertical_margin = config.effective_vertical_margin as usize;

    // Phase 2 Step 16: bounded channel activation (natural backpressure via blocking_send).
    let (tx, mut rx) = mpsc::channel::<Event>(EVENT_CHANNEL_CAP);
    let _input_handle = core_input::spawn_input_thread(tx.clone());

    // Command line now stored within EditorState (Refactor R1 Step 2)

    // Sticky visual column for vertical motions (None until first j/k).
    let mut sticky_visual_col: Option<usize> = None;

    // Instantiate render engine (stateful from Refactor R2 Step 2 - cursor span meta retained).
    let mut render_engine = RenderEngine::new();

    // Initial render so the user sees content before pressing a key.
    if let Err(e) = render(&mut render_engine, &state) {
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
                    let before_line = state.position.line; // capture before dispatch for heuristic
                    let dr = dispatch(act, &mut state, &mut sticky_visual_col, &observers);
                    if dr.dirty {
                        // Heuristic mapping (Phase 2 Step 17): if line changed -> Lines(range of that line),
                        // if only cursor moved within same line -> CursorOnly, else fallback Full.
                        let after_line = state.position.line;
                        if before_line != after_line {
                            scheduler.mark(RenderDelta::Lines(after_line..after_line + 1));
                        } else {
                            // Cursor move or intra-line edit; we cannot cheaply know if text mutated -> use Lines for safety if in Insert, else CursorOnly.
                            if matches!(state.mode, Mode::Insert) {
                                scheduler.mark(RenderDelta::Lines(after_line..after_line + 1));
                            } else {
                                scheduler.mark(RenderDelta::CursorOnly);
                            }
                        }
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
        // Expire ephemeral status if needed (breadth-first synchronous check)
        if state.tick_ephemeral() {
            scheduler.mark(RenderDelta::StatusLine);
        }
        // Auto-scroll (Phase 2 Step 8): keep cursor visible.
        if let Ok((_, h)) = crossterm::terminal::size() {
            let text_height = if h > 0 { (h - 1) as usize } else { 0 };
            if state.auto_scroll(text_height) {
                // scrolling changes visible lines -> conservatively mark full for now
                scheduler.mark(RenderDelta::Full);
            }
        }
        // Ask scheduler if a redraw is needed this cycle.
        if let Some(decision) = scheduler.consume() {
            // TODO(Phase 3): Switch on decision.semantic to attempt partial paints.
            // match decision.semantic { ... } retaining Full fallback.
            if let Err(e) = render(&mut render_engine, &state) {
                error!(?e, "render error");
            }
            // NOTE: decision.effective is ignored (Phase 2 always Full) but kept for future flexibility.
            let _ = decision; // suppress unused warning until Phase 3.
        }
    }
    // Guard drop will restore terminal.
    info!("Oxidized Phase 0 exiting");
    Ok(())
}

fn render(engine: &mut RenderEngine, state: &EditorState) -> Result<()> {
    use crossterm::terminal::size;
    let (w, h) = size()?;
    let span = tracing::info_span!("render_cycle");
    let _e = span.enter();
    // Refactor R2 Step 2: stateful engine retains cursor span metadata (still full render).
    engine.render_full(state, w, h)
}

fn translate_key_wrapper(mode: Mode, pending_command: &str, key: &KeyEvent) -> Option<Action> {
    core_actions::translate_key(mode, pending_command, key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_actions::{EditKind, ModeChange};
    use core_render::render_engine::{RenderEngine, build_content_frame};
    use core_text::Buffer;
    use tokio::sync::mpsc; // imported after refactor R2 Step 1

    #[tokio::test]
    async fn bounded_channel_capacity_blocking() {
        // Tiny channel to exercise pending send; we manually receive to free space.
        let (tx, mut rx) = mpsc::channel::<Event>(2);
        tx.send(Event::RenderRequested).await.unwrap();
        tx.send(Event::RenderRequested).await.unwrap();
        // Next send would await until a recv occurs. Spawn a task to release one slot.
        let tx2 = tx.clone();
        let send_fut = tokio::spawn(async move {
            tx2.send(Event::RenderRequested).await.unwrap();
        });
        // Yield to ensure task is pending, then receive one event to free space.
        tokio::task::yield_now().await;
        rx.recv().await.unwrap();
        // Now the blocked send should complete.
        send_fut.await.unwrap();
        assert!(rx.recv().await.is_some());
    }

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

    // --- Software cursor tests (Phase 2 Step 12) ---
    #[test]
    fn cursor_ascii_single_width() {
        let mut state = mk_state("abc");
        state.position.line = 0;
        state.position.byte = 1; // 'b'
        // Use engine to get full frame (includes cursor overlay) for assertions
        let mut eng = RenderEngine::new();
        let _ = eng.render_full(&state, 20, 4); // renders to terminal; for tests we instead rebuild content
        let frame = build_content_frame(&state, 20, 4); // cursor overlay no longer part of content-only frame
        let idx = 1; // (y * width) + x
        let cell = frame.cells[idx];
        assert_eq!(cell.ch, 'b');
    }

    #[test]
    fn cursor_wide_emoji() {
        let mut state = mk_state("a😀\n"); // trailing newline for consistency
        // Position cursor at start of emoji cluster
        let line = state.active_buffer().line(0).unwrap();
        let emoji_byte = line.char_indices().find(|(_, c)| *c == '😀').unwrap().0;
        state.position.line = 0;
        state.position.byte = emoji_byte;
        let frame = build_content_frame(&state, 20, 4);
        // Visual column after 'a' is 1
        let base_col = 1usize; // leading cell of wide emoji
        let idx_first = base_col; // row 0 so direct index
        let first = frame.cells[idx_first];
        assert_eq!(first.ch, '😀');
        // Second cell of span should be a space but still flagged
        let idx_second = base_col + 1;
        let second = frame.cells[idx_second];
        assert_eq!(second.ch, ' ');
    }

    #[test]
    fn cursor_combining_sequence() {
        // e + combining acute accent (width 1 cluster)
        let mut state = mk_state("\u{0065}\u{0301}x\n");
        state.position.line = 0;
        state.position.byte = 0; // start of cluster
        let frame = build_content_frame(&state, 20, 4);
        let idx = 0;
        let cell = frame.cells[idx];
        assert_eq!(cell.ch, 'e'); // first scalar of cluster
        // Next cell should be the 'x' not flagged (cursor span width=1)
        let idx_next = 1;
        let next = frame.cells[idx_next];
        assert_eq!(next.ch, 'x');
        assert!(!next.flags.contains(core_render::CellFlags::CURSOR));
    }

    #[test]
    fn cursor_end_of_line_blank_cell() {
        let mut state = mk_state("abc\n");
        state.position.line = 0;
        // Move to end (after 'c')
        state.position.byte = state
            .active_buffer()
            .line(0)
            .unwrap()
            .trim_end_matches(['\n', '\r'])
            .len();
        let frame = build_content_frame(&state, 20, 4);
        // Visual column == 3
        let idx = 3;
        let cell = frame.cells[idx];
        assert_eq!(cell.ch, ' '); // synthesized space
    }
}
