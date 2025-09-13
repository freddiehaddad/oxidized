//! Oxidized entrypoint – Phase 0 skeleton.

use anyhow::Result;
use core_actions::{Action, EditKind, ModeChange, MotionKind};
use core_events::{CommandEvent, Event, InputEvent, KeyCode, KeyEvent};
use core_render::status::{StatusContext, build_status};
use core_render::{Frame, Renderer};
use core_state::EditorState;
use core_state::Mode;
use core_terminal::{CrosstermBackend, TerminalBackend};
use core_text::Buffer;
use core_text::{grapheme, motion};
use tokio::sync::mpsc;
use tracing::{error, info};

// --- Render Scheduler Stub (Task 9.8) ---
// Breadth-first placeholder: encapsulates 'dirty' tracking and full-frame redraw policy.
// Future phases will extend this with coalescing, debounce timers, and diff-based damage sets.
struct RenderScheduler {
    dirty: bool,
}

impl RenderScheduler {
    fn new() -> Self {
        Self { dirty: false }
    }
    fn mark_dirty(&mut self) {
        self.dirty = true;
    }
    fn consume_dirty(&mut self) -> bool {
        if self.dirty {
            self.dirty = false;
            true
        } else {
            false
        }
    }
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

    let buffer = Buffer::from_str(
        "welcome",
        "Welcome to Oxidized (Phase 0)\nPress :q to quit.",
    )?;
    let mut state = EditorState::new(buffer);
    // Async unbounded channel (single consumer main loop). Input thread forwards blocking crossterm events.
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
    while let Some(event) = rx.recv().await {
        // NOTE: No polling – loop wakes only on incoming events from channel.
        match event {
            Event::Input(InputEvent::CtrlC) => {
                info!("shutdown");
                break;
            }
            Event::Input(InputEvent::Key(k)) => match k.code {
                KeyCode::Colon => {
                    state.command_line.begin();
                }
                KeyCode::Char(c) => {
                    // Use translator for motions/commands (Insert edits not yet wired)
                    if let Some(act) = translate_key_wrapper(
                        state.mode,
                        state.command_line.buffer(),
                        &KeyEvent {
                            code: KeyCode::Char(c),
                            mods: k.mods,
                        },
                    ) {
                        let dr = dispatch(act, &mut state, &mut sticky_visual_col);
                        if dr.dirty {
                            scheduler.mark_dirty();
                        }
                        if dr.quit {
                            break;
                        }
                    }
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
                    if let Some(act) =
                        translate_key_wrapper(state.mode, state.command_line.buffer(), &k)
                    {
                        let dr = dispatch(act, &mut state, &mut sticky_visual_col);
                        if dr.dirty {
                            scheduler.mark_dirty();
                        }
                        if dr.quit {
                            break;
                        }
                    }
                }
                KeyCode::Enter => {
                    if let Some(act) =
                        translate_key_wrapper(state.mode, state.command_line.buffer(), &k)
                    {
                        let dr = dispatch(act, &mut state, &mut sticky_visual_col);
                        if dr.dirty {
                            scheduler.mark_dirty();
                        }
                        if dr.quit {
                            break;
                        }
                    } else {
                        // Fallback: if Enter while in command mode with :q
                        if state.command_line.buffer() == ":q" {
                            break;
                        }
                        // Otherwise clear any pending command input (e.g., empty or cancelled)
                        if state.command_line.is_active() {
                            state.command_line.clear();
                            scheduler.mark_dirty();
                        }
                    }
                }
                KeyCode::Backspace => {
                    if let Some(act) =
                        translate_key_wrapper(state.mode, state.command_line.buffer(), &k)
                    {
                        let dr = dispatch(act, &mut state, &mut sticky_visual_col);
                        if dr.dirty {
                            scheduler.mark_dirty();
                        }
                        if dr.quit {
                            break;
                        }
                    }
                }
                KeyCode::Esc => {
                    // Route through translator so Insert mode Escape triggers ModeChange::LeaveInsert
                    if let Some(act) =
                        translate_key_wrapper(state.mode, state.command_line.buffer(), &k)
                    {
                        let dr = dispatch(act, &mut state, &mut sticky_visual_col);
                        if dr.dirty {
                            scheduler.mark_dirty();
                        }
                        if dr.quit {
                            break;
                        }
                    } else {
                        // Fallback: clear any pending (e.g. stray command buffer) and mark dirty
                        state.command_line.clear();
                        scheduler.mark_dirty();
                    }
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

// Helper to apply a simple motion function without violating borrow rules.
// Extracts the position out temporarily to ensure the buffer (&self) borrow ends
// before we mutably borrow the position.
fn apply_motion<F>(state: &mut EditorState, f: F)
where
    F: Fn(&core_text::Buffer, &mut core_text::Position),
{
    use std::mem;
    let buf_ptr: *const core_text::Buffer = state.active_buffer(); // immutable borrow ends after this line
    // Move position out, operate, then move back.
    let mut pos = mem::replace(&mut state.position, core_text::Position::origin());
    unsafe {
        f(&*buf_ptr, &mut pos);
    }
    state.position = pos;
}

fn apply_vertical_motion<F>(state: &mut EditorState, sticky: Option<usize>, f: F) -> Option<usize>
where
    F: Fn(&core_text::Buffer, &mut core_text::Position, Option<usize>) -> Option<usize>,
{
    use std::mem;
    let buf_ptr: *const core_text::Buffer = state.active_buffer();
    let mut pos = mem::replace(&mut state.position, core_text::Position::origin());
    let new_sticky = unsafe { f(&*buf_ptr, &mut pos, sticky) };
    state.position = pos;
    new_sticky
}

// --- Dispatcher Skeleton (Task 9.7) ---
struct DispatchResult {
    dirty: bool,
    quit: bool,
}

fn dispatch(
    action: Action,
    state: &mut EditorState,
    sticky_visual_col: &mut Option<usize>,
) -> DispatchResult {
    match action {
        Action::Motion(kind) => {
            let before_line = state.position.line;
            let before_byte = state.position.byte;
            match kind {
                MotionKind::Left => {
                    apply_motion(state, motion::left);
                    *sticky_visual_col = None;
                }
                MotionKind::Right => {
                    apply_motion(state, motion::right);
                    *sticky_visual_col = None;
                }
                MotionKind::LineStart => {
                    apply_motion(state, motion::line_start);
                    *sticky_visual_col = None;
                }
                MotionKind::LineEnd => {
                    apply_motion(state, motion::line_end);
                    *sticky_visual_col = None;
                }
                MotionKind::Up => {
                    *sticky_visual_col =
                        apply_vertical_motion(state, *sticky_visual_col, motion::up);
                }
                MotionKind::Down => {
                    *sticky_visual_col =
                        apply_vertical_motion(state, *sticky_visual_col, motion::down);
                }
                MotionKind::WordForward => {
                    apply_motion(state, motion::word_forward);
                    *sticky_visual_col = None;
                }
                MotionKind::WordBackward => {
                    apply_motion(state, motion::word_backward);
                    *sticky_visual_col = None;
                }
            }
            let moved = before_line != state.position.line || before_byte != state.position.byte;
            DispatchResult {
                dirty: moved,
                quit: false,
            }
        }
        Action::ModeChange(mc) => {
            match mc {
                ModeChange::EnterInsert => {
                    // Starting a new Insert session always ends any prior run defensively.
                    state.end_insert_coalescing();
                    state.mode = Mode::Insert;
                }
                ModeChange::LeaveInsert => {
                    // Leaving Insert ends coalescing run boundary.
                    state.end_insert_coalescing();
                    state.mode = Mode::Normal;
                }
            }
            DispatchResult {
                dirty: true,
                quit: false,
            }
        }
        Action::CommandInput(ch) => {
            if ch == '\u{08}' {
                state.command_line.backspace();
            } else {
                state.command_line.push_char(ch);
            }
            DispatchResult {
                dirty: true,
                quit: false,
            }
        }
        Action::CommandExecute(cmd) => {
            if cmd == ":q" {
                return DispatchResult {
                    dirty: true,
                    quit: true,
                };
            }
            // Empty string or unrecognized: clear for now.
            state.command_line.clear();
            DispatchResult {
                dirty: true,
                quit: false,
            }
        }
        Action::Edit(kind) => {
            match kind {
                EditKind::InsertGrapheme(g) => {
                    if matches!(state.mode, Mode::Insert) {
                        let span = tracing::trace_span!("edit_insert", grapheme = %g);
                        let _enter = span.enter();
                        state.begin_insert_coalescing();
                        let mut pos = state.position;
                        state.active_buffer_mut().insert_grapheme(&mut pos, &g);
                        state.position = pos;
                        return DispatchResult {
                            dirty: true,
                            quit: false,
                        };
                    }
                }
                EditKind::InsertNewline => {
                    if matches!(state.mode, Mode::Insert) {
                        let span = tracing::trace_span!("edit_newline");
                        let _enter = span.enter();
                        // Newline is a coalescing boundary: ensure snapshot for prior run, then end it.
                        state.begin_insert_coalescing(); // if first action in run, capture pre-edit
                        let mut pos = state.position;
                        state.active_buffer_mut().insert_newline(&mut pos);
                        state.position = pos;
                        // End current run so subsequent inserts start a new snapshot sequence.
                        state.end_insert_coalescing();
                        return DispatchResult {
                            dirty: true,
                            quit: false,
                        };
                    }
                }
                EditKind::Backspace => {
                    if matches!(state.mode, Mode::Insert) {
                        let span = tracing::trace_span!("edit_backspace");
                        let _enter = span.enter();
                        // Backspace stays within current coalescing run (does NOT end it).
                        // If this is the first edit in a new run, capture snapshot.
                        state.begin_insert_coalescing();
                        let mut pos = state.position;
                        state.active_buffer_mut().delete_grapheme_before(&mut pos);
                        state.position = pos;
                        return DispatchResult {
                            dirty: true,
                            quit: false,
                        };
                    }
                }
                EditKind::DeleteUnder => {
                    if matches!(state.mode, Mode::Normal) {
                        let span = tracing::trace_span!("edit_delete_under");
                        let _enter = span.enter();
                        // Always discrete snapshot per Task 6 simple spec (no coalescing for now).
                        state.push_discrete_edit_snapshot();
                        let mut pos = state.position;
                        state.active_buffer_mut().delete_grapheme_at(&mut pos);
                        state.position = pos;
                        return DispatchResult {
                            dirty: true,
                            quit: false,
                        };
                    }
                }
            }
            DispatchResult {
                dirty: false,
                quit: false,
            }
        }
        Action::Undo | Action::Redo => {
            let _e = if matches!(action, Action::Undo) {
                tracing::trace_span!("undo").entered()
            } else {
                tracing::trace_span!("redo").entered()
            };
            let applied = match action {
                Action::Undo => state.undo(),
                Action::Redo => state.redo(),
                _ => false,
            };
            DispatchResult {
                dirty: applied,
                quit: false,
            }
        }
        Action::Quit => DispatchResult {
            dirty: false,
            quit: true,
        },
    }
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
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut state,
            &mut sticky,
        );
        // Insert 'a'
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("a".to_string())),
            &mut state,
            &mut sticky,
        );
        assert_eq!(state.active_buffer().line(0).unwrap(), "a");
        assert_eq!(state.undo_stack.len(), 1, "expected first snapshot");
        // Newline
        dispatch(
            Action::Edit(EditKind::InsertNewline),
            &mut state,
            &mut sticky,
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
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut state,
            &mut sticky,
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("a".to_string())),
            &mut state,
            &mut sticky,
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("b".to_string())),
            &mut state,
            &mut sticky,
        );
        assert_eq!(state.active_buffer().line(0).unwrap(), "ab");
        assert_eq!(state.undo_stack.len(), 1, "still single run snapshot");
        // Backspace
        dispatch(Action::Edit(EditKind::Backspace), &mut state, &mut sticky);
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
        );
        // Undo should revert entire sequence (buffer empty)
        assert!(dispatch(Action::Undo, &mut state, &mut sticky).dirty);
        assert_eq!(state.active_buffer().line(0).unwrap(), "");
    }

    #[test]
    fn normal_mode_delete_under_single() {
        let mut state = mk_state("abc");
        let mut sticky = None;
        // Delete 'a'
        dispatch(Action::Edit(EditKind::DeleteUnder), &mut state, &mut sticky);
        assert_eq!(state.active_buffer().line(0).unwrap(), "bc");
        assert_eq!(state.undo_stack.len(), 1, "snapshot pushed for delete");
        // Undo
        assert!(dispatch(Action::Undo, &mut state, &mut sticky).dirty);
        assert_eq!(state.active_buffer().line(0).unwrap(), "abc");
    }

    #[test]
    fn normal_mode_delete_under_multiple_and_undo() {
        let mut state = mk_state("abcd");
        let mut sticky = None;
        // Delete 'a'
        dispatch(Action::Edit(EditKind::DeleteUnder), &mut state, &mut sticky);
        // Delete 'b' (originally 'c', now at index 0 after first delete)
        dispatch(Action::Edit(EditKind::DeleteUnder), &mut state, &mut sticky);
        assert_eq!(state.active_buffer().line(0).unwrap(), "cd");
        assert_eq!(state.undo_stack.len(), 2, "two discrete snapshots");
        // Undo last -> should restore to "bcd" (?) Actually sequence: start abcd -> after first delete bcd -> after second delete cd. Undo should return to bcd.
        assert!(dispatch(Action::Undo, &mut state, &mut sticky).dirty);
        assert_eq!(state.active_buffer().line(0).unwrap(), "bcd");
        // Undo again -> original
        assert!(dispatch(Action::Undo, &mut state, &mut sticky).dirty);
        assert_eq!(state.active_buffer().line(0).unwrap(), "abcd");
    }
}
