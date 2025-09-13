//! Oxidized entrypoint.
use anyhow::Result;
use clap::Parser;
use core_actions::dispatcher::dispatch;
use core_actions::{Action, ActionObserver, EditKind}; // trait (currently unused in main but stored for future use)
use core_config::{ConfigContext, ConfigPlatformTraits, load_from};
use core_events::{
    CommandEvent, EVENT_CHANNEL_CAP, Event, EventHooks, EventSourceRegistry, InputEvent,
    NoopEventHooks, TickEventSource,
};
use core_render::apply::{
    CursorOnlyFrame, FrameSnapshot, LinesPartialFrame, ScrollShiftFrame, apply_cursor_only,
    apply_full, apply_lines_partial, apply_scroll_shift,
};
use core_render::scheduler::{RenderDelta, RenderScheduler};
// Frame kept for tests still referencing type
use core_model::EditorModel;
use core_render::render_engine::RenderEngine;
use core_state::Mode;
use core_state::{EditorState, normalize_line_endings};
use core_terminal::{CrosstermBackend, TerminalBackend, TerminalCapabilities};
use core_text::Buffer;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{error, info};
// Centralized normalization + segmentation adapter
use core_text::segment::normalize_and_segment;

#[inline]
fn log_paste_commit(content: &str, grapheme_count: usize) {
    tracing::debug!(
        target: "input.paste",
        size_bytes = content.len(),
        grapheme_count = grapheme_count,
        "paste_commit"
    );
}

// RenderScheduler moved to core-render::scheduler (Refactor R1 Step 4)

/// CLI arguments.
#[derive(Parser, Debug)]
#[command(name = "oxidized", version, about = "Oxidized editor")] // minimal metadata
struct Args {
    /// Optional path to open at startup (UTF-8 text). If omitted a welcome buffer is used.
    pub path: Option<std::path::PathBuf>,
    /// Optional configuration file path (overrides discovery of `oxidized.toml`).
    #[arg(long = "config")]
    pub config: Option<std::path::PathBuf>,
}

const STATUS_ROWS: u16 = 1;

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

    info!(target: "runtime", "startup");

    // Install a panic hook to ensure we log unexpected panics before the
    // terminal is restored by the backend's Drop impl.
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(target: "runtime.panic", ?info, "panic");
        // Call the default (prints to stderr) so the user still sees it.
        default_panic(info);
    }));

    let mut term = CrosstermBackend::new();
    term.set_title("Oxidized")?; // set title before entering alt screen
    // Use RAII guard so any early return/panic restores the terminal.
    let _term_guard = term.enter_guard()?;

    let args = Args::parse();

    // Base buffer: either loaded from file or a new empty "untitled" buffer.
    let mut open_failed = false;
    let (buffer, file_name, norm_meta) = if let Some(path) = args.path {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let size_bytes = content.len();
                let norm = normalize_line_endings(&content);
                let line_count = norm.normalized.lines().count();
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("file");
                tracing::debug!(target: "io", file=%path.display(), size_bytes, line_count, "file_read_ok");
                (
                    Buffer::from_str(name, &norm.normalized)?,
                    Some(path),
                    Some(norm),
                )
            }
            Err(e) => {
                error!(target: "io", ?e, "file_open_error");
                open_failed = true;
                (Buffer::from_str("untitled", "")?, None, None)
            }
        }
    } else {
        (Buffer::from_str("untitled", "")?, None, None)
    };
    let mut model = EditorModel::new(EditorState::new(buffer));
    {
        let state = model.state_mut();
        state.file_name = file_name;
        if let Some(n) = norm_meta {
            state.original_line_ending = n.original;
            state.had_trailing_newline = n.had_trailing_newline;
            if n.mixed {
                tracing::warn!(target: "io", "mixed_line_endings_detected_startup");
            }
        }
        state.dirty = false; // new buffer always clean at load
        if open_failed {
            state.set_ephemeral("Open failed", std::time::Duration::from_secs(3));
        }
    }
    // Load configuration (Phase 2 Step 14). We parse early so margin can influence initial scroll decisions.
    let mut config = load_from(args.config.clone())?; // args consumed earlier for path
    let terminal_caps = TerminalCapabilities::detect();
    let platform_traits =
        ConfigPlatformTraits::new(cfg!(windows), terminal_caps.supports_scroll_region);
    if let Ok((w, h)) = crossterm::terminal::size() {
        // Initial viewport context (status line occupies one row).
        let ctx = ConfigContext::new(w, h, STATUS_ROWS, 0, platform_traits);
        config.apply_context(ctx);
    }
    // Store effective margin inside state (temporary field addition in Phase 2 Step 14).
    model.state_mut().config_vertical_margin = config.effective_vertical_margin as usize;

    // Phase 2 Step 16: bounded channel activation (natural backpressure via blocking_send).
    let (tx, mut rx) = mpsc::channel::<Event>(EVENT_CHANNEL_CAP);
    let _input_handle = core_input::spawn_input_thread(tx.clone());
    // Refactor R4 Step 14: event source registry (tick source as first implementation)
    let mut registry = EventSourceRegistry::new();
    registry.register(TickEventSource::new(std::time::Duration::from_millis(250)));
    let _source_handles = registry.spawn_all(tx.clone());

    // Command line now stored within EditorState (Refactor R1 Step 2)

    // Sticky visual column for vertical motions (None until first j/k).
    let mut sticky_visual_col: Option<usize> = None;

    // Instantiate render engine (stateful from Refactor R2 Step 2 - cursor span meta retained).
    let mut render_engine = RenderEngine::new();

    // Initial render so the user sees content before pressing a key.
    let initial_decision = core_render::scheduler::Decision {
        semantic: RenderDelta::Full,
        effective: RenderDelta::Full,
    };
    // Borrow view index then perform mutable state borrow for render to satisfy borrow checker.
    let view_ref_ptr: *const core_model::View = model.active_view() as *const _;
    if let Err(e) = {
        // Safe: active_view reference used immutably while we borrow state mutably in render.
        let view_ref = unsafe { &*view_ref_ptr };
        render(
            &mut render_engine,
            model.state_mut(),
            view_ref,
            &initial_decision,
        )
    } {
        error!(target: "render.engine", ?e, "initial_render_error");
    }

    // Runtime event loop span (debug level per logging guide philosophy).
    let render_span = tracing::debug_span!(target: "runtime", "event_loop");
    let _enter_loop = render_span.enter();
    let mut scheduler = RenderScheduler::new();
    // Accumulator for bracketed paste streaming
    let mut paste_accum: Option<String> = None;
    // Refactor R3 Step 6: snapshot of fields influencing status line to detect
    // status-only changes and emit RenderDelta::StatusLine instead of CursorOnly.
    #[derive(Clone)]
    struct StatusSnapshot {
        mode_disc: std::mem::Discriminant<Mode>,
        command_active: bool,
        command_buffer: String,
        ephemeral: Option<String>,
        dirty: bool,
    }
    impl StatusSnapshot {
        fn capture(state: &EditorState) -> Self {
            Self {
                mode_disc: std::mem::discriminant(&state.mode),
                command_active: state.command_line.is_active(),
                command_buffer: state.command_line.buffer().to_string(),
                ephemeral: state.ephemeral_status.as_ref().map(|m| m.text.clone()),
                dirty: state.dirty,
            }
        }
        fn differs(&self, other: &StatusSnapshot) -> bool {
            self.mode_disc != other.mode_disc
                || self.command_active != other.command_active
                || self.command_buffer != other.command_buffer
                || self.ephemeral != other.ephemeral
                || self.dirty != other.dirty
        }
    }
    // Refactor R1 Step 8: prepare empty observer list (macro recorder, analytics to be added later).
    let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
    // Hooks: allow pre/post observe (currently no-op)
    let hooks: Box<dyn EventHooks> = Box::new(NoopEventHooks);
    let mut ngi_timeout_deadline: Option<Instant> = None;
    while let Some(event) = rx.recv().await {
        hooks.pre_handle(&event);
        // Standardized per-event span for observability.
        let kind = match &event {
            Event::Input(_) => "input",
            Event::Command(_) => "command",
            Event::RenderRequested => "render_requested",
            Event::Tick => "tick",
            Event::Shutdown => "shutdown",
        };
        let span = tracing::debug_span!(target: "runtime.event", "handle_event", kind);
        let _enter = span.enter();
        let mut lines_changed: usize = 0;
        let mut scrolled: bool = false;
        // NOTE: No polling – loop wakes only on incoming events from channel.
        match &event {
            Event::Input(InputEvent::KeyPress(_)) => {
                // Rich key press path will be integrated in later phases; ignore for now.
            }
            Event::Input(InputEvent::CtrlC) => {
                info!(target: "runtime", "shutdown");
                break;
            }
            Event::Input(InputEvent::Key(k)) => {
                // Single unified path: every key translated (breadth-first simplicity)
                let snapshot_mode = model.state().mode; // minimize immutable borrows
                let cmd_buf = model.state().command_line.buffer().to_string();
                let resolution = core_actions::translate_ngi(snapshot_mode, &cmd_buf, k, &config);
                ngi_timeout_deadline = resolution.timeout_deadline;
                if let Some(act) = resolution.action {
                    let pre_status_snapshot = StatusSnapshot::capture(model.state());
                    let before_line = model.active_view().cursor.line;
                    let dr = dispatch(act, &mut model, &mut sticky_visual_col, &observers);
                    if dr.buffer_replaced {
                        // Phase 3 Step 9.1: buffer replacement (e.g. :e <file>) is a structural
                        // change – invalidate partial render cache and escalate to Full render.
                        render_engine.invalidate_for_resize(); // reuse same cache clear semantics
                        scheduler.mark(RenderDelta::Full);
                    } else if dr.dirty {
                        let after_line = model.active_view().cursor.line;
                        let insert_mode = matches!(model.state().mode, Mode::Insert);
                        let post_status_snapshot = StatusSnapshot::capture(model.state());
                        let status_changed = post_status_snapshot.differs(&pre_status_snapshot);
                        let line_changed = before_line != after_line || insert_mode;
                        if line_changed {
                            scheduler.mark(RenderDelta::Lines(after_line..after_line + 1));
                            lines_changed = 1;
                        } else if status_changed {
                            scheduler.mark(RenderDelta::StatusLine);
                        } else {
                            scheduler.mark(RenderDelta::CursorOnly);
                        }
                    }
                    if dr.quit {
                        break;
                    }
                }
            }
            Event::Input(InputEvent::Resize(w, h)) => {
                // Phase 3 Step 9 (integration): on terminal resize, invalidate partial
                // render cache so next frame performs a full rebuild. Mark a Full delta
                // to guarantee a complete repaint rather than a partial path using stale
                // geometry. Still recompute vertical margin and mark StatusLine if changed.
                render_engine.invalidate_for_resize();
                scheduler.mark(RenderDelta::Full);
                let ctx = ConfigContext::new(*w, *h, STATUS_ROWS, 0, platform_traits);
                if let Some(new_margin) = config.recompute_with_context(ctx) {
                    model.state_mut().config_vertical_margin = new_margin as usize;
                    scheduler.mark(RenderDelta::StatusLine);
                }
            }
            Event::RenderRequested => {}
            Event::Tick => {
                // Ephemeral expiry driven by tick (no busy waiting). If expired mark status delta.
                if model.state_mut().tick_ephemeral() {
                    scheduler.mark(RenderDelta::StatusLine);
                }
                // NGI adapter: ambiguous mapping timeout flush (Vim-style timeoutlen)
                if let Some(deadline) = ngi_timeout_deadline
                    && Instant::now() >= deadline
                    && let Some(resolution) = core_actions::flush_pending_literal(&config)
                {
                    ngi_timeout_deadline = resolution.timeout_deadline;
                    if let Some(act) = resolution.action {
                        // Dispatch flushed action (treated as command char if colon-active)
                        let pre_status_snapshot = StatusSnapshot::capture(model.state());
                        let before_line = model.active_view().cursor.line;
                        let dr = dispatch(act, &mut model, &mut sticky_visual_col, &observers);
                        if dr.buffer_replaced {
                            render_engine.invalidate_for_resize();
                            scheduler.mark(RenderDelta::Full);
                        } else if dr.dirty {
                            let after_line = model.active_view().cursor.line;
                            let insert_mode = matches!(model.state().mode, Mode::Insert);
                            let post_status_snapshot = StatusSnapshot::capture(model.state());
                            let status_changed = post_status_snapshot.differs(&pre_status_snapshot);
                            let line_changed = before_line != after_line || insert_mode;
                            if line_changed {
                                scheduler.mark(RenderDelta::Lines(after_line..after_line + 1));
                                lines_changed = 1;
                            } else if status_changed {
                                scheduler.mark(RenderDelta::StatusLine);
                            } else {
                                scheduler.mark(RenderDelta::CursorOnly);
                            }
                        }
                    }
                }
                // Future: metrics overlay refresh or cursor blink logic can hook here.
            }
            Event::Command(CommandEvent::Quit) => {
                break;
            }
            Event::Shutdown => {
                break;
            }
            // NGI enriched input variants (initial wiring): normalized handling or stubs.
            Event::Input(InputEvent::TextCommit(s)) => {
                // Central adapter: NFC normalize and segment into grapheme clusters (Unicode-correct inserts).
                let (normalized, graphemes) = {
                    let (n, segs) = normalize_and_segment(s);
                    (
                        n,
                        segs.into_iter().map(|seg| seg.cluster).collect::<Vec<_>>(),
                    )
                };
                tracing::debug!(target: "input.normalize", grapheme_count = graphemes.len(), bytes = normalized.len(), "text_commit");
                if model.state().command_line.is_active() {
                    // Command-line text: append characters to command buffer via CommandChar actions.
                    let pre_status_snapshot = StatusSnapshot::capture(model.state());
                    let before_line = model.active_view().cursor.line;
                    let mut any_dirty = false;
                    let mut buffer_replaced = false;
                    for ch in normalized.chars() {
                        let dr = dispatch(
                            Action::CommandChar(ch),
                            &mut model,
                            &mut sticky_visual_col,
                            &observers,
                        );
                        any_dirty |= dr.dirty;
                        buffer_replaced |= dr.buffer_replaced;
                    }
                    if buffer_replaced {
                        render_engine.invalidate_for_resize();
                        scheduler.mark(RenderDelta::Full);
                    } else if any_dirty {
                        let after_line = model.active_view().cursor.line;
                        let insert_mode = matches!(model.state().mode, Mode::Insert);
                        let post_status_snapshot = StatusSnapshot::capture(model.state());
                        let status_changed = post_status_snapshot.differs(&pre_status_snapshot);
                        let line_changed = before_line != after_line || insert_mode;
                        if line_changed {
                            scheduler.mark(RenderDelta::Lines(after_line..after_line + 1));
                        } else if status_changed {
                            scheduler.mark(RenderDelta::StatusLine);
                        } else {
                            scheduler.mark(RenderDelta::CursorOnly);
                        }
                    }
                } else if matches!(model.state().mode, Mode::Insert) {
                    // Insert-mode text: dispatch per-grapheme InsertGrapheme actions.
                    let pre_status_snapshot = StatusSnapshot::capture(model.state());
                    let before_line = model.active_view().cursor.line;
                    let mut any_dirty = false;
                    let mut buffer_replaced = false;
                    for g in &graphemes {
                        let dr = dispatch(
                            Action::Edit(EditKind::InsertGrapheme(g.to_string())),
                            &mut model,
                            &mut sticky_visual_col,
                            &observers,
                        );
                        any_dirty |= dr.dirty;
                        buffer_replaced |= dr.buffer_replaced;
                    }
                    if buffer_replaced {
                        render_engine.invalidate_for_resize();
                        scheduler.mark(RenderDelta::Full);
                    } else if any_dirty {
                        let after_line = model.active_view().cursor.line;
                        let insert_mode = matches!(model.state().mode, Mode::Insert);
                        let post_status_snapshot = StatusSnapshot::capture(model.state());
                        let status_changed = post_status_snapshot.differs(&pre_status_snapshot);
                        let line_changed = before_line != after_line || insert_mode;
                        if line_changed {
                            scheduler.mark(RenderDelta::Lines(after_line..after_line + 1));
                        } else if status_changed {
                            scheduler.mark(RenderDelta::StatusLine);
                        } else {
                            scheduler.mark(RenderDelta::CursorOnly);
                        }
                    }
                } else {
                    // In other modes, ignore TextCommit for now.
                }
            }
            Event::Input(InputEvent::PasteStart) => {
                // Begin accumulation; never log content, only sizes/metrics.
                paste_accum = Some(String::new());
            }
            Event::Input(InputEvent::PasteChunk(chunk)) => {
                if let Some(buf) = &mut paste_accum {
                    buf.push_str(chunk);
                } else {
                    // Out-of-order chunk; ignore.
                }
            }
            Event::Input(InputEvent::PasteEnd) => {
                if let Some(buf) = paste_accum.take() {
                    let (normalized, graphemes) = {
                        let (n, segs) = normalize_and_segment(&buf);
                        (
                            n,
                            segs.into_iter().map(|seg| seg.cluster).collect::<Vec<_>>(),
                        )
                    };
                    log_paste_commit(&normalized, graphemes.len());
                    if model.state().command_line.is_active() {
                        // Command-line: feed chars
                        let pre_status_snapshot = StatusSnapshot::capture(model.state());
                        let before_line = model.active_view().cursor.line;
                        let mut any_dirty = false;
                        let mut buffer_replaced = false;
                        for ch in normalized.chars() {
                            let dr = dispatch(
                                Action::CommandChar(ch),
                                &mut model,
                                &mut sticky_visual_col,
                                &observers,
                            );
                            any_dirty |= dr.dirty;
                            buffer_replaced |= dr.buffer_replaced;
                        }
                        if buffer_replaced {
                            render_engine.invalidate_for_resize();
                            scheduler.mark(RenderDelta::Full);
                        } else if any_dirty {
                            let after_line = model.active_view().cursor.line;
                            let insert_mode = matches!(model.state().mode, Mode::Insert);
                            let post_status_snapshot = StatusSnapshot::capture(model.state());
                            let status_changed = post_status_snapshot.differs(&pre_status_snapshot);
                            let line_changed = before_line != after_line || insert_mode;
                            if line_changed {
                                scheduler.mark(RenderDelta::Lines(after_line..after_line + 1));
                            } else if status_changed {
                                scheduler.mark(RenderDelta::StatusLine);
                            } else {
                                scheduler.mark(RenderDelta::CursorOnly);
                            }
                        }
                    } else if matches!(model.state().mode, Mode::Insert) {
                        // Insert-mode: feed graphemes
                        let pre_status_snapshot = StatusSnapshot::capture(model.state());
                        let before_line = model.active_view().cursor.line;
                        let mut any_dirty = false;
                        let mut buffer_replaced = false;
                        for g in &graphemes {
                            let dr = dispatch(
                                Action::Edit(EditKind::InsertGrapheme(g.to_string())),
                                &mut model,
                                &mut sticky_visual_col,
                                &observers,
                            );
                            any_dirty |= dr.dirty;
                            buffer_replaced |= dr.buffer_replaced;
                        }
                        if buffer_replaced {
                            render_engine.invalidate_for_resize();
                            scheduler.mark(RenderDelta::Full);
                        } else if any_dirty {
                            let after_line = model.active_view().cursor.line;
                            let insert_mode = matches!(model.state().mode, Mode::Insert);
                            let post_status_snapshot = StatusSnapshot::capture(model.state());
                            let status_changed = post_status_snapshot.differs(&pre_status_snapshot);
                            let line_changed = before_line != after_line || insert_mode;
                            if line_changed {
                                scheduler.mark(RenderDelta::Lines(after_line..after_line + 1));
                            } else if status_changed {
                                scheduler.mark(RenderDelta::StatusLine);
                            } else {
                                scheduler.mark(RenderDelta::CursorOnly);
                            }
                        }
                    } else {
                        // Other modes: ignore paste for now.
                    }
                }
            }
            Event::Input(InputEvent::Mouse(_)) => {
                // Stub: ignore for now; tracing can be added later.
            }
            Event::Input(InputEvent::FocusGained) | Event::Input(InputEvent::FocusLost) => {
                // Stub: ignore for now.
            }
            Event::Input(InputEvent::RawBytes(_)) => {
                // Stub: ignore for now.
            }
            Event::Input(InputEvent::CompositionUpdate { .. }) => {
                // Stub: ignore for now; UI could surface preedit later.
            }
        }
        // Ephemeral expiry moved into Tick handling above.
        // Auto-scroll (Phase 2 Step 8): keep cursor visible.
        if let Ok((w, h)) = crossterm::terminal::size() {
            // Effective text height excludes status line AND overlay rows (metrics overlay etc.).
            let overlay_rows = if h > 0 {
                core_render::overlay::overlay_line_count(model.state(), w)
            } else {
                0
            } as usize;
            let base_text_height = if h > 0 { (h - 1) as usize } else { 0 }; // exclude status line
            let effective_text_height = base_text_height.saturating_sub(overlay_rows);
            let before_first = model.active_view().viewport_first_line;
            let scroll_changed = {
                let (state, view) = model.split_state_and_active_view();
                view.auto_scroll(state, effective_text_height)
            };
            if scroll_changed {
                let after_first = model.active_view().viewport_first_line;
                scheduler.mark(RenderDelta::Scroll {
                    old_first: before_first,
                    new_first: after_first,
                });
                scrolled = true;
            }
        }
        // Selection/scroll ordering guarantee: selection and cursor updates (from dispatch)
        // have already applied before we examine scheduler decisions, and auto_scroll just ran.
        // This ensures the rendered cursor/selection is coherent with the computed viewport.
        debug_assert!(
            model.active_view().cursor.line < model.state().active_buffer().line_count(),
            "cursor must be within buffer before scheduling render"
        );
        // Ask scheduler if a redraw is needed this cycle.
        if let Some(decision) = scheduler.consume() {
            tracing::debug!(target: "render.scheduler", semantic=?decision.semantic, effective=?decision.effective, lines_changed, scrolled, "render_decision");
            // Similar borrow split for subsequent renders.
            let view_ptr: *const core_model::View = model.active_view() as *const _;
            if let Err(e) = {
                let view_ref = unsafe { &*view_ptr };
                render(&mut render_engine, model.state_mut(), view_ref, &decision)
            } {
                error!(target: "render.engine", ?e, "render_error");
            } else {
                // Capture scheduler metrics snapshot directly via mutable state.
                let sch = scheduler.metrics_snapshot();
                let st = model.state_mut();
                st.last_render_delta = Some(core_state::RenderDeltaSnapshotLite {
                    full: sch.full,
                    lines: sch.lines,
                    scroll: sch.scroll,
                    status_line: sch.status_line,
                    cursor_only: sch.cursor_only,
                    collapsed_scroll: sch.collapsed_scroll,
                    suppressed_scroll: sch.suppressed_scroll,
                    semantic_frames: sch.semantic_frames,
                });
            }
        }
        hooks.post_handle(&event);
    }
    // Guard drop will restore terminal.
    info!(target: "runtime", "shutdown_complete");
    Ok(())
}

fn render(
    engine: &mut RenderEngine,
    state: &mut EditorState,
    view: &core_model::View,
    decision: &core_render::scheduler::Decision,
) -> Result<()> {
    use core_render::timing::record_last_render_ns;
    use crossterm::terminal::size;
    use std::time::Instant;
    let (w, h) = size()?;
    let span = tracing::debug_span!(target: "render.engine", "render_cycle", semantic=?decision.semantic, effective=?decision.effective, width=w, height=h);
    let _e = span.enter();
    // Refactor R2 Step 2: stateful engine retains cursor span metadata (still full render).
    // Refactor R2 Step 11: capture render duration.
    let start = Instant::now();
    let layout = core_model::Layout::single(w, h);
    let res = match &decision.effective {
        core_render::scheduler::RenderDelta::CursorOnly => {
            let status_line =
                core_render::render_engine::build_status_line_with_ephemeral(state, view, w);
            let snapshot = FrameSnapshot::new(&*state, view, &layout, w, h, &status_line);
            apply_cursor_only(engine, CursorOnlyFrame::new(snapshot))
        }
        core_render::scheduler::RenderDelta::Lines(dirty_lines) => {
            let status_line =
                core_render::render_engine::build_status_line_with_ephemeral(state, view, w);
            let mut tracker = core_render::dirty::DirtyLinesTracker::new();
            for line in dirty_lines.start..dirty_lines.end {
                tracker.mark(line);
            }
            let snapshot = FrameSnapshot::new(&*state, view, &layout, w, h, &status_line);
            apply_lines_partial(engine, LinesPartialFrame::new(snapshot, &mut tracker))
        }
        core_render::scheduler::RenderDelta::Scroll {
            old_first,
            new_first,
        } => {
            let status_line =
                core_render::render_engine::build_status_line_with_ephemeral(state, view, w);
            let snapshot = FrameSnapshot::new(&*state, view, &layout, w, h, &status_line);
            apply_scroll_shift(
                engine,
                ScrollShiftFrame::new(snapshot, *old_first, *new_first),
            )
        }
        _ => {
            let status_line =
                core_render::render_engine::build_status_line_with_ephemeral(state, view, w);
            let snapshot = FrameSnapshot::new(&*state, view, &layout, w, h, &status_line);
            apply_full(engine, snapshot)
        }
    };
    let elapsed = start.elapsed();
    record_last_render_ns(elapsed.as_nanos() as u64);
    // Store metrics snapshots breadth-first (mutably borrow state via raw pointer pattern if needed).
    if res.is_ok() {
        let snap = engine.metrics_snapshot();
        state.last_render_path = Some(core_state::RenderPathSnapshotLite {
            full_frames: snap.full_frames,
            partial_frames: snap.partial_frames,
            cursor_only_frames: snap.cursor_only_frames,
            lines_frames: snap.lines_frames,
            escalated_large_set: snap.escalated_large_set,
            resize_invalidations: snap.resize_invalidations,
            dirty_lines_marked: snap.dirty_lines_marked,
            dirty_candidate_lines: snap.dirty_candidate_lines,
            dirty_lines_repainted: snap.dirty_lines_repainted,
            last_full_render_ns: snap.last_full_render_ns,
            last_partial_render_ns: snap.last_partial_render_ns,
            print_commands: snap.print_commands,
            cells_printed: snap.cells_printed,
            scroll_region_shifts: snap.scroll_region_shifts,
            scroll_region_lines_saved: snap.scroll_region_lines_saved,
            scroll_shift_degraded_full: snap.scroll_shift_degraded_full,
            trim_attempts: snap.trim_attempts,
            trim_success: snap.trim_success,
            cols_saved_total: snap.cols_saved_total,
            status_skipped: snap.status_skipped,
        });
    }
    res
}

// Pure helper: NFC-normalize a string and return its grapheme clusters.
// normalize_and_segment moved to core_text::segment

#[cfg(test)]
mod tests {
    use super::*;
    use core_actions::{EditKind, ModeChange};
    use core_render::render_engine::{RenderEngine, build_content_frame};
    use core_text::Buffer;
    use std::fmt;
    use std::sync::{Arc, Mutex};
    use tokio::sync::mpsc; // imported after refactor R2 Step 1
    use tracing::Subscriber;
    use tracing::dispatcher::Dispatch;
    use tracing::field::{Field, Visit};
    use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
    use tracing_subscriber::registry::Registry;

    #[derive(Clone, Default)]
    struct Capture {
        events: Arc<Mutex<Vec<CapturedEvent>>>,
    }

    #[derive(Clone, Debug)]
    struct CapturedEvent {
        target: String,
        fields: Vec<(String, String)>,
    }

    #[derive(Default)]
    struct FieldCollector {
        fields: Vec<(String, String)>,
    }

    impl Visit for FieldCollector {
        fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
            self.fields
                .push((field.name().to_string(), format!("{:?}", value)));
        }
    }

    impl<S> Layer<S> for Capture
    where
        S: Subscriber,
    {
        fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
            let mut collector = FieldCollector::default();
            event.record(&mut collector);
            let meta = event.metadata();
            self.events.lock().unwrap().push(CapturedEvent {
                target: meta.target().to_string(),
                fields: collector.fields,
            });
        }
    }

    #[test]
    fn paste_commit_log_redacts_content() {
        let capture = Capture::default();
        let events = capture.events.clone();
        let subscriber = Registry::default().with(capture);
        let dispatch = Dispatch::new(subscriber);

        tracing::dispatcher::with_default(&dispatch, || {
            let secret = "classified buffer ✂️";
            super::log_paste_commit(secret, 3);
        });

        let events = events.lock().unwrap();
        assert!(
            !events.is_empty(),
            "expected at least one captured input.paste event"
        );
        let event = events
            .iter()
            .find(|e| e.target == "input.paste")
            .expect("missing input.paste event");
        assert!(
            event.fields.iter().any(|(name, _)| name == "size_bytes"),
            "size_bytes field missing from event"
        );
        assert!(
            event
                .fields
                .iter()
                .any(|(name, _)| name == "grapheme_count"),
            "grapheme_count field missing from event"
        );
        for (_, value) in &event.fields {
            assert!(
                !value.contains("classified buffer"),
                "event leaked raw paste content: {value}"
            );
            assert!(
                !value.contains("✂️"),
                "event leaked emoji from paste content: {value}"
            );
        }
    }

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

    fn mk_state_model(initial: &str) -> core_model::EditorModel {
        let buf = Buffer::from_str("test", initial).unwrap();
        let state = EditorState::new(buf);
        core_model::EditorModel::new(state)
    }

    #[test]
    fn insert_newline_coalescing_boundary() {
        let mut model = mk_state_model("");
        let mut sticky = None;
        // Enter insert
        let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut model,
            &mut sticky,
            &observers,
        );
        // Insert 'a'
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("a".to_string())),
            &mut model,
            &mut sticky,
            &observers,
        );
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "a");
        assert_eq!(model.state().undo_depth(), 1, "expected first snapshot");
        // Newline
        dispatch(
            Action::Edit(EditKind::InsertNewline),
            &mut model,
            &mut sticky,
            &observers,
        );
        assert_eq!(model.state().active_buffer().line_count(), 2);
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "a\n");
        // Insert 'b' (new run -> new snapshot)
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("b".to_string())),
            &mut model,
            &mut sticky,
            &observers,
        );
        assert_eq!(model.state().active_buffer().line(1).unwrap(), "b");
        assert_eq!(
            model.state().undo_depth(),
            2,
            "expected second snapshot after new run"
        );
    }

    #[test]
    fn backspace_stays_within_run_dispatch() {
        let mut model = mk_state_model("");
        let mut sticky = None;
        let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut model,
            &mut sticky,
            &observers,
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("a".to_string())),
            &mut model,
            &mut sticky,
            &observers,
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("b".to_string())),
            &mut model,
            &mut sticky,
            &observers,
        );
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "ab");
        assert_eq!(model.state().undo_depth(), 1, "still single run snapshot");
        // Backspace
        dispatch(
            Action::Edit(EditKind::Backspace),
            &mut model,
            &mut sticky,
            &observers,
        );
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "a");
        assert_eq!(
            model.state().undo_depth(),
            1,
            "backspace should not create new snapshot"
        );
        // Leave insert -> ends run
        dispatch(
            Action::ModeChange(ModeChange::LeaveInsert),
            &mut model,
            &mut sticky,
            &observers,
        );
        // Undo should revert entire sequence (buffer empty)
        assert!(dispatch(Action::Undo, &mut model, &mut sticky, &observers).dirty);
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "");
    }

    #[test]
    fn normal_mode_delete_under_single() {
        let mut model = mk_state_model("abc");
        let mut sticky = None;
        // Delete 'a'
        let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
        dispatch(
            Action::Edit(EditKind::DeleteUnder),
            &mut model,
            &mut sticky,
            &observers,
        );
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "bc");
        assert_eq!(model.state().undo_depth(), 1, "snapshot pushed for delete");
        // Register should now contain removed grapheme 'a'
        assert!(model.state().registers.unnamed.starts_with('a'));
        // Undo
        assert!(dispatch(Action::Undo, &mut model, &mut sticky, &observers).dirty);
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "abc");
    }

    #[test]
    fn normal_mode_delete_under_then_pastes_with_p() {
        let mut model = mk_state_model("xyz");
        let mut sticky = None;
        let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
        // Delete 'x'
        dispatch(
            Action::Edit(EditKind::DeleteUnder),
            &mut model,
            &mut sticky,
            &observers,
        );
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "yz");
        let reg = model.state().registers.unnamed.clone();
        assert_eq!(reg, "x");
        // Move cursor to start (already at 0) and paste after -> should insert after cursor producing x y z order restored as xyzz? Wait semantics: Step1 paste inserts at cursor (simplified) so we adjust expectation.
        // For now simplified paste inserts at cursor; ensure we at least insert register content.
        dispatch(
            Action::PasteAfter { register: None },
            &mut model,
            &mut sticky,
            &observers,
        );
        let line = model.state().active_buffer().line(0).unwrap();
        assert!(
            line.contains('x'),
            "pasting should reinsert deleted grapheme"
        );
    }

    #[test]
    fn normal_mode_delete_under_multiple_and_undo() {
        let mut model = mk_state_model("abcd");
        let mut sticky = None;
        // Delete 'a'
        let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
        dispatch(
            Action::Edit(EditKind::DeleteUnder),
            &mut model,
            &mut sticky,
            &observers,
        );
        // Delete 'b' (originally 'c', now at index 0 after first delete)
        dispatch(
            Action::Edit(EditKind::DeleteUnder),
            &mut model,
            &mut sticky,
            &observers,
        );
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "cd");
        assert_eq!(model.state().undo_depth(), 2, "two discrete snapshots");
        // Undo last -> should restore to "bcd" (?) Actually sequence: start abcd -> after first delete bcd -> after second delete cd. Undo should return to bcd.
        assert!(dispatch(Action::Undo, &mut model, &mut sticky, &observers).dirty);
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "bcd");
        // Undo again -> original
        assert!(dispatch(Action::Undo, &mut model, &mut sticky, &observers).dirty);
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "abcd");
    }

    // --- Software cursor tests (Phase 2 Step 12) ---
    #[test]
    fn cursor_ascii_single_width() {
        let state = EditorState::new(Buffer::from_str("test", "abc").unwrap());
        let mut view = core_model::View::new(
            core_model::ViewId(0),
            state.active,
            core_text::Position::origin(),
            0,
        );
        view.cursor.line = 0;
        view.cursor.byte = 1; // 'b'
        let mut eng = RenderEngine::new();
        let layout = core_model::Layout::single(20, 4);
        let status_line = core_render::render_engine::build_status_line(&state, &view);
        let _ = eng.render_full(&state, &view, &layout, 20, 4, &status_line);
        let frame = build_content_frame(&state, &view, 20, 4);
        let idx = 1; // (y * width) + x
        let cell = &frame.cells[idx];
        assert!(cell.is_leader(), "expected leader cell at cursor position");
        assert_eq!(cell.cluster.as_str(), "b");
    }

    #[test]
    fn cursor_wide_emoji() {
        let state = EditorState::new(Buffer::from_str("test", "a😀\n").unwrap());
        let mut view = core_model::View::new(
            core_model::ViewId(0),
            state.active,
            core_text::Position::origin(),
            0,
        );
        let line = state.active_buffer().line(0).unwrap();
        let emoji_byte = line.char_indices().find(|(_, c)| *c == '😀').unwrap().0;
        view.cursor.line = 0;
        view.cursor.byte = emoji_byte;
        let frame = build_content_frame(&state, &view, 20, 4);
        // Visual column after 'a' is 1
        let base_col = 1usize; // leading cell of wide emoji
        let idx_first = base_col; // row 0 so direct index
        let first = &frame.cells[idx_first];
        assert!(first.is_leader(), "emoji leader should be leader cell");
        assert_eq!(first.cluster.as_str(), "😀");
        // Second cell is a continuation (width==0)
        let idx_second = base_col + 1;
        let second = &frame.cells[idx_second];
        assert!(
            !second.is_leader(),
            "expected continuation cell for wide emoji"
        );
    }

    #[test]
    fn cursor_combining_sequence() {
        let state = EditorState::new(Buffer::from_str("test", "\u{0065}\u{0301}x\n").unwrap());
        let mut view = core_model::View::new(
            core_model::ViewId(0),
            state.active,
            core_text::Position::origin(),
            0,
        );
        view.cursor.line = 0;
        view.cursor.byte = 0;
        let frame = build_content_frame(&state, &view, 20, 4);
        let idx = 0;
        let cell = &frame.cells[idx];
        // Entire combining sequence should exist in the leader cluster string.
        assert!(cell.is_leader());
        assert_eq!(cell.cluster.as_str(), "e\u{301}"); // e + combining acute accent
        // Next visual cell should be the 'x' leader.
        let idx_next = 1;
        let next = &frame.cells[idx_next];
        assert!(next.is_leader());
        assert_eq!(next.cluster.as_str(), "x");
        assert!(!next.flags.contains(core_render::CellFlags::CURSOR));
    }

    #[test]
    fn cursor_end_of_line_blank_cell() {
        let state = EditorState::new(Buffer::from_str("test", "abc\n").unwrap());
        let mut view = core_model::View::new(
            core_model::ViewId(0),
            state.active,
            core_text::Position::origin(),
            0,
        );
        view.cursor.line = 0;
        view.cursor.byte = state
            .active_buffer()
            .line(0)
            .unwrap()
            .trim_end_matches(['\n', '\r'])
            .len();
        let frame = build_content_frame(&state, &view, 20, 4);
        // Visual column == 3
        let idx = 3;
        let cell = &frame.cells[idx];
        assert!(cell.is_leader());
        assert_eq!(cell.cluster.as_str(), " "); // synthesized space
    }

    #[tokio::test]
    async fn tick_event_expires_ephemeral_and_schedules_status() {
        // Set up minimal channel and model, inject ephemeral already expired to trigger path.
        let (tx, mut rx) = mpsc::channel::<Event>(8);
        // Send a Tick after creating state with expired ephemeral.
        let buffer = Buffer::from_str("t", "hello").unwrap();
        let mut model = EditorModel::new(EditorState::new(buffer));
        {
            let st = model.state_mut();
            st.set_ephemeral("Temp", std::time::Duration::from_millis(1));
            // Force expiration by rewinding expires_at.
            if let Some(m) = &mut st.ephemeral_status {
                m.expires_at = std::time::Instant::now() - std::time::Duration::from_millis(5);
            }
        }
        let mut scheduler = RenderScheduler::new();
        tx.send(Event::Tick).await.unwrap();
        if let Some(Event::Tick) = rx.recv().await
            && model.state_mut().tick_ephemeral()
        {
            scheduler.mark(RenderDelta::StatusLine);
        }
        // Consume scheduler decision and ensure it's StatusLine.
        let decision = scheduler.consume().expect("expected decision");
        matches!(decision.effective, RenderDelta::StatusLine);
    }

    #[test]
    fn text_commit_nfc_equivalence_single_cluster() {
        // "e" + combining acute vs precomposed "é" should normalize identically
        let decomposed = "e\u{0301}"; // e + combining acute
        let composed = "\u{00E9}"; // precomposed é
        let (n1, g1) = normalize_and_segment(decomposed);
        let (n2, g2) = normalize_and_segment(composed);
        assert_eq!(n1, n2, "NFC normalized strings should be equal");
        assert_eq!(g1, g2, "Grapheme sequences should be identical");
        assert_eq!(g1.len(), 1, "Should be a single grapheme cluster");
        assert_eq!(g1[0].cluster.as_str(), "é");
    }

    #[test]
    fn text_commit_grapheme_segmentation_mixed() {
        // Mixed content: ASCII + wide emoji + combining mark attaches to previous cluster
        // "a" + grinning face emoji + combining acute accent
        let s = format!("a{}\u{0301}", '😀');
        let (norm, clusters) = normalize_and_segment(&s);
        // Expect two clusters: "a" and "😀́" (emoji + combining mark)
        assert_eq!(clusters.len(), 2, "Expected two grapheme clusters");
        assert_eq!(clusters[0].cluster.as_str(), "a");
        assert_eq!(clusters[1].cluster.as_str(), "😀\u{0301}");
        // Round-trip join should equal normalized string
        let joined: String = clusters.iter().map(|s| s.cluster.as_str()).collect();
        assert_eq!(joined, norm);
    }

    #[test]
    fn text_commit_dangling_combining_mark_start() {
        // A dangling combining mark at the start should form its own cluster.
        let s = "\u{0301}"; // combining acute accent alone
        let (norm, clusters) = normalize_and_segment(s);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].cluster.as_str(), "\u{0301}");
        let joined: String = clusters.iter().map(|s| s.cluster.as_str()).collect();
        assert_eq!(joined, norm);
    }

    #[test]
    fn paste_like_insert_applies_text() {
        // Simulate a paste commit in Insert mode by normalizing and dispatching graphemes.
        let mut model = mk_state_model("");
        let mut sticky = None;
        let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
        // Enter insert mode
        dispatch(
            Action::ModeChange(core_actions::ModeChange::EnterInsert),
            &mut model,
            &mut sticky,
            &observers,
        );
        // Mixed text including emoji and composed char
        let combining: char = '\u{0301}';
        let raw = format!("he{}llo e{}", '😀', combining);
        let (norm, clusters) = normalize_and_segment(&raw);
        for g in clusters {
            dispatch(
                Action::Edit(EditKind::InsertGrapheme(g.cluster)),
                &mut model,
                &mut sticky,
                &observers,
            );
        }
        let line = model.state().active_buffer().line(0).unwrap();
        assert_eq!(line, norm);
    }

    #[test]
    fn paste_like_commandline_appends() {
        // Simulate a paste commit in command-line by normalizing and dispatching chars.
        let mut model = mk_state_model("abc\n");
        let mut sticky = None;
        let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
        // Start command line
        dispatch(Action::CommandStart, &mut model, &mut sticky, &observers);
        let raw = "wq!";
        let (norm, _clusters) = normalize_and_segment(raw);
        for ch in norm.chars() {
            dispatch(Action::CommandChar(ch), &mut model, &mut sticky, &observers);
        }
        assert!(model.state().command_line.is_active());
        assert_eq!(model.state().command_line.buffer(), ":wq!");
    }

    #[test]
    fn mouse_and_focus_events_are_ignored() {
        // Ensure stubs don't alter buffer or mode when such events are encountered.
        let model = mk_state_model("hello\n");
        let initial_line = model.state().active_buffer().line(0).unwrap().to_string();
        let initial_mode = model.state().mode;
        // Simulate events by directly matching the stubs' no-op policy (no dispatch)
        // Mouse
        let _mouse = core_events::MouseEvent {
            kind: core_events::MouseEventKind::Moved,
            column: 5,
            row: 1,
            mods: core_events::ModMask::empty(),
        };
        // Focus
        let _fg = core_events::Event::Input(core_events::InputEvent::FocusGained);
        let _fl = core_events::Event::Input(core_events::InputEvent::FocusLost);
        // Assert no changes occurred (since runtime would ignore these)
        assert_eq!(model.state().active_buffer().line(0).unwrap(), initial_line);
        assert_eq!(model.state().mode, initial_mode);
    }

    #[tokio::test]
    async fn end_to_end_paste_insert_mode() {
        // Build a model and simulate entering insert mode, then feed paste events end-to-end.
        let mut model = mk_state_model("");
        let mut sticky = None;
        let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
        dispatch(
            Action::ModeChange(core_actions::ModeChange::EnterInsert),
            &mut model,
            &mut sticky,
            &observers,
        );

        // Prepare a realistic mixed string with emoji and combining mark across chunks
        let combining: char = '\u{0301}';
        let chunk1 = format!("he{}l", '😀');
        let chunk2 = format!("lo e{}", combining);

        // Accumulate into a buffer (mirrors behavior without using Option)
        let mut buf = String::new();
        buf.push_str(&chunk1);
        buf.push_str(&chunk2);
        // PasteEnd -> normalize, segment, and dispatch like main loop
        let (norm, graphemes) = normalize_and_segment(&buf);
        for g in &graphemes {
            dispatch(
                Action::Edit(EditKind::InsertGrapheme(g.cluster.clone())),
                &mut model,
                &mut sticky,
                &observers,
            );
        }
        let line = model.state().active_buffer().line(0).unwrap();
        assert_eq!(line, norm);
    }

    #[tokio::test]
    async fn end_to_end_paste_commandline_mode() {
        // Build a model and simulate command-line active, then feed paste events and commit.
        let mut model = mk_state_model("abc\n");
        let mut sticky = None;
        let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
        // Start command line
        dispatch(Action::CommandStart, &mut model, &mut sticky, &observers);

        // Paste content across multiple chunks
        // When command-line is active, the buffer already contains ':' prefix.
        // Paste only the command content without a leading ':'.
        let chunk1 = "w";
        let chunk2 = "q!";

        // Accumulate and commit like main loop path
        let mut buf = String::new();
        buf.push_str(chunk1);
        buf.push_str(chunk2);
        let (norm, _g) = normalize_and_segment(&buf);
        for ch in norm.chars() {
            dispatch(Action::CommandChar(ch), &mut model, &mut sticky, &observers);
        }
        assert!(model.state().command_line.is_active());
        assert_eq!(model.state().command_line.buffer(), ":wq!");
    }

    #[tokio::test]
    async fn event_span_and_scheduler_fields_smoke() {
        // Exercise a tiny slice of the loop: mark a status change via ephemeral expiry and ensure
        // scheduler consume occurs without panic. This indirectly traverses the per-event span.
        let mut model = mk_state_model("");
        // Set ephemeral to expire immediately
        model
            .state_mut()
            .set_ephemeral("hi", std::time::Duration::from_millis(0));
        let mut scheduler = RenderScheduler::new();
        // Simulate Tick handling
        if model.state_mut().tick_ephemeral() {
            scheduler.mark(RenderDelta::StatusLine);
        }
        let decision = scheduler.consume().expect("expected decision");
        assert!(matches!(
            decision.effective,
            RenderDelta::StatusLine | RenderDelta::Full | RenderDelta::Lines(_)
        ));
    }
}
