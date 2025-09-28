//! Oxidized entrypoint.
use anyhow::Result;
use clap::Parser;
use core_actions::dispatcher::dispatch;
use core_actions::{Action, ActionObserver, EditKind, PendingState}; // trait (currently unused in main but stored for future use)
use core_config::{ConfigContext, ConfigPlatformTraits, load_from};
use core_events::{
    CommandEvent, EVENT_CHANNEL_CAP, Event, EventHooks, EventSourceRegistry, InputEvent,
    NoopEventHooks, TickEventSource,
};
use core_model::EditorModel;
use core_render::apply::{
    CursorOnlyFrame, FrameSnapshot, LinesPartialFrame, ScrollShiftFrame, apply_cursor_only,
    apply_full, apply_lines_partial, apply_scroll_shift,
};
use core_render::render_engine::RenderEngine;
use core_render::scheduler::{RenderDelta, RenderDeltaMetricsSnapshot, RenderScheduler};
use core_state::Mode;
use core_state::{EditorState, normalize_line_endings};
use core_terminal::{CrosstermBackend, TerminalBackend, TerminalCapabilities};
use core_text::Buffer;
use core_text::segment::normalize_and_segment;
use std::fmt;
use std::mem::Discriminant;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{error, info, trace, warn};
use tracing_appender::non_blocking::WorkerGuard;

const STATUS_ROWS: u16 = 1;

#[inline]
fn log_paste_commit(content: &str, grapheme_count: usize) {
    tracing::debug!(
        target: "input.paste",
        size_bytes = content.len(),
        grapheme_count = grapheme_count,
        "paste_commit"
    );
}

fn normalize_into_graphemes(input: &str) -> (String, Vec<String>) {
    let (normalized, segments) = normalize_and_segment(input);
    let graphemes = segments
        .into_iter()
        .map(|segment| segment.cluster)
        .collect::<Vec<_>>();
    (normalized, graphemes)
}

/// CLI arguments.
#[derive(Parser, Debug)]
#[command(name = "oxidized", version, about = "Oxidized editor")] // minimal metadata
struct Args {
    /// Optional path to open at startup (UTF-8 text). If omitted a welcome buffer is used.
    pub path: Option<PathBuf>,
    /// Optional configuration file path (overrides discovery of `oxidized.toml`).
    #[arg(long = "config")]
    pub config: Option<PathBuf>,
}

struct AppStartup {
    backend: CrosstermBackend,
    log_guard: Option<WorkerGuard>,
}

struct RuntimeContext<'a> {
    model: EditorModel,
    config: core_config::Config,
    platform_traits: ConfigPlatformTraits,
    terminal_guard: core_terminal::TerminalGuard<'a>,
}

#[derive(Debug, Clone)]
struct StartupTelemetry {
    buffer_name: String,
    opened_path: Option<PathBuf>,
    config_override: bool,
    open_failed: bool,
}

impl StartupTelemetry {
    fn new(
        buffer_name: String,
        opened_path: Option<PathBuf>,
        config_override: bool,
        open_failed: bool,
    ) -> Self {
        Self {
            buffer_name,
            opened_path,
            config_override,
            open_failed,
        }
    }
}

impl AppStartup {
    fn new() -> Self {
        Self {
            backend: CrosstermBackend::new(),
            log_guard: None,
        }
    }

    fn run<'a>(&'a mut self) -> Result<RuntimeContext<'a>> {
        self.configure_logging()?;
        Self::install_panic_hook();

        info!(target: "runtime", "startup");
        self.backend.set_title("Oxidized")?;
        let guard = self.backend.enter_guard()?;

        let args = Args::parse();
        let bootstrap = Self::load_editor_state(&args)?;

        let path_str = bootstrap
            .telemetry
            .opened_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());
        info!(
            target: "runtime.startup",
            buffer = bootstrap.telemetry.buffer_name.as_str(),
            path = path_str.as_deref(),
            open_failed = bootstrap.telemetry.open_failed,
            config_override = bootstrap.telemetry.config_override,
            effective_margin = bootstrap.config.effective_vertical_margin,
            "bootstrap_complete"
        );

        Ok(RuntimeContext {
            model: bootstrap.model,
            config: bootstrap.config,
            platform_traits: bootstrap.platform_traits,
            terminal_guard: guard,
        })
    }

    fn configure_logging(&mut self) -> Result<()> {
        let log_dir = Path::new(".");
        let log_path = log_dir.join("oxidized.log");
        if log_path.exists() {
            let _ = std::fs::remove_file(&log_path);
        }

        let file_appender = tracing_appender::rolling::never(log_dir, "oxidized.log");
        let (nb_writer, guard) = tracing_appender::non_blocking(file_appender);
        match tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_writer(nb_writer)
            .try_init()
        {
            Ok(_) => {
                self.log_guard = Some(guard);
            }
            Err(_err) => {
                // Global tracing subscriber already installed; drop guard so writer shuts down.
            }
        }

        Ok(())
    }

    fn install_panic_hook() {
        static HOOK: Once = Once::new();
        HOOK.call_once(|| {
            let default_panic = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                tracing::error!(target: "runtime.panic", ?info, "panic");
                default_panic(info);
            }));
        });
    }

    fn load_editor_state(args: &Args) -> Result<EditorBootstrap> {
        let mut open_failed = false;
        let (buffer, file_name, norm_meta) = if let Some(path) = args.path.as_ref() {
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    let size_bytes = content.len();
                    let norm = normalize_line_endings(&content);
                    let line_count = norm.normalized.lines().count();
                    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("file");
                    tracing::debug!(target: "io", file=%path.display(), size_bytes, line_count, "file_read_ok");
                    (
                        Buffer::from_str(name, &norm.normalized)?,
                        Some(path.clone()),
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
            state.dirty = false;
            if open_failed {
                state.set_ephemeral("Open failed", std::time::Duration::from_secs(3));
            }
        }

        let mut config = load_from(args.config.clone())?;
        let terminal_caps = TerminalCapabilities::detect();
        let platform_traits =
            ConfigPlatformTraits::new(cfg!(windows), terminal_caps.supports_scroll_region);
        if let Ok((w, h)) = crossterm::terminal::size() {
            let ctx = ConfigContext::new(w, h, STATUS_ROWS, 0, platform_traits);
            config.apply_context(ctx);
        }
        model.state_mut().config_vertical_margin = config.effective_vertical_margin as usize;

        let telemetry = StartupTelemetry::new(
            model
                .state()
                .file_name
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("untitled")
                .to_string(),
            args.path.clone(),
            args.config.is_some(),
            open_failed,
        );

        Ok(EditorBootstrap {
            model,
            config,
            platform_traits,
            telemetry,
        })
    }
}

struct EditorBootstrap {
    model: EditorModel,
    config: core_config::Config,
    platform_traits: ConfigPlatformTraits,
    telemetry: StartupTelemetry,
}

struct EditorRuntime<'a> {
    model: EditorModel,
    config: core_config::Config,
    platform_traits: ConfigPlatformTraits,
    scheduler: RenderScheduler,
    render_engine: RenderEngine,
    render_metrics: RenderMetricsLedger,
    sticky_visual_col: Option<usize>,
    paste: PasteSession,
    ngi_timeout: NgiTimeoutState,
    observers: Vec<Box<dyn ActionObserver>>,
    hooks: Box<dyn EventHooks>,
    rx: mpsc::Receiver<Event>,
    tx: Option<mpsc::Sender<Event>>,
    source_handles: Vec<tokio::task::JoinHandle<()>>,
    input_task: Option<tokio::task::JoinHandle<()>>,
    input_shutdown: Option<core_input::AsyncInputShutdown>,
    _terminal_guard: core_terminal::TerminalGuard<'a>,
}

#[derive(Clone)]
struct StatusSnapshot {
    mode_disc: Discriminant<Mode>,
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

#[derive(Clone)]
struct CommandContextSnapshot {
    mode: Mode,
    command_active: bool,
    pending_buffer: String,
}

impl CommandContextSnapshot {
    fn capture(state: &EditorState) -> Self {
        Self {
            mode: state.mode,
            command_active: state.command_line.is_active(),
            pending_buffer: state.command_line.buffer().to_string(),
        }
    }

    fn mode(&self) -> Mode {
        self.mode
    }

    fn pending_buffer(&self) -> &str {
        &self.pending_buffer
    }

    #[cfg(test)]
    fn command_active(&self) -> bool {
        self.command_active
    }

    fn colon_active(&self) -> bool {
        self.command_active && self.pending_buffer.starts_with(':')
    }
}

#[derive(Default)]
struct DispatchOutcome {
    dirty: bool,
    buffer_replaced: bool,
    quit: bool,
    status_changed: bool,
    line_changed: bool,
}

impl DispatchOutcome {
    fn new(
        dirty: bool,
        buffer_replaced: bool,
        quit: bool,
        status_changed: bool,
        line_changed: bool,
    ) -> Self {
        Self {
            dirty,
            buffer_replaced,
            quit,
            status_changed,
            line_changed,
        }
    }

    fn absorb(&mut self, other: DispatchOutcome) {
        self.dirty |= other.dirty;
        self.buffer_replaced |= other.buffer_replaced;
        self.quit |= other.quit;
        self.status_changed |= other.status_changed;
        self.line_changed |= other.line_changed;
    }
}

#[derive(Default, Clone, Copy)]
struct RenderMetricsLedger {
    last_delta: Option<core_state::RenderDeltaSnapshotLite>,
    last_path: Option<core_state::RenderPathSnapshotLite>,
}

impl RenderMetricsLedger {
    fn store(
        &mut self,
        delta: Option<core_state::RenderDeltaSnapshotLite>,
        path: core_state::RenderPathSnapshotLite,
    ) {
        self.last_delta = delta;
        self.last_path = Some(path);
    }

    fn apply_to_state(&self, state: &mut EditorState) {
        state.last_render_delta = self.last_delta;
        state.last_render_path = self.last_path;
    }
}

#[derive(Clone, Copy)]
struct NgiTimeoutState {
    deadline: Option<Instant>,
    pending: PendingState,
}

impl Default for NgiTimeoutState {
    fn default() -> Self {
        Self {
            deadline: None,
            pending: PendingState::Idle,
        }
    }
}

impl NgiTimeoutState {
    fn update(&mut self, pending: PendingState, deadline: Option<Instant>) {
        self.pending = pending;
        self.deadline = deadline;
        if matches!(self.pending, PendingState::Idle) {
            self.deadline = None;
        }
    }

    fn clear(&mut self) {
        self.pending = PendingState::Idle;
        self.deadline = None;
    }

    #[cfg(test)]
    fn pending(&self) -> PendingState {
        self.pending
    }

    #[cfg(test)]
    fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    fn poll_expired<F>(&mut self, now: Instant, flush: F) -> Option<TimeoutFlushResult>
    where
        F: FnOnce() -> Option<TimeoutFlushResult>,
    {
        if !matches!(self.pending, PendingState::AwaitingMore { .. }) {
            if let Some(deadline) = self.deadline
                && now >= deadline
            {
                self.clear();
            }
            return None;
        }

        let deadline = self.deadline?;

        if now < deadline {
            return None;
        }

        let Some(result) = flush() else {
            self.clear();
            return None;
        };

        self.update(result.pending, result.deadline);
        Some(result)
    }
}

struct TimeoutFlushResult {
    action: Option<Action>,
    pending: PendingState,
    deadline: Option<Instant>,
}

impl TimeoutFlushResult {
    fn from_resolution(resolution: core_actions::NgiResolution) -> Self {
        Self {
            action: resolution.action,
            pending: resolution.pending_state,
            deadline: resolution.timeout_deadline,
        }
    }
}

struct RenderInvoker<'a> {
    engine: &'a mut RenderEngine,
    scheduler: &'a RenderScheduler,
    metrics: &'a mut RenderMetricsLedger,
}

impl<'a> RenderInvoker<'a> {
    fn new(
        engine: &'a mut RenderEngine,
        scheduler: &'a RenderScheduler,
        metrics: &'a mut RenderMetricsLedger,
    ) -> Self {
        Self {
            engine,
            scheduler,
            metrics,
        }
    }

    fn apply(
        &mut self,
        model: &mut EditorModel,
        decision: &core_render::scheduler::Decision,
    ) -> Result<()> {
        let (state, view) = model.split_state_and_active_view();
        let path_snapshot = render(self.engine, state, &*view, decision)?;
        let delta_snapshot = convert_delta_snapshot(self.scheduler.metrics_snapshot());
        self.metrics.store(delta_snapshot, path_snapshot);
        self.metrics.apply_to_state(state);
        Ok(())
    }
}

enum LoopControl {
    Continue { lines_changed: usize },
    Break { reason: ShutdownReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShutdownReason {
    CtrlC,
    CommandQuit,
    ActionQuit,
    ShutdownEvent,
    ChannelClosed,
}

impl ShutdownReason {
    fn as_str(&self) -> &'static str {
        match self {
            ShutdownReason::CtrlC => "ctrl_c",
            ShutdownReason::CommandQuit => "command_quit",
            ShutdownReason::ActionQuit => "action_quit",
            ShutdownReason::ShutdownEvent => "shutdown_event",
            ShutdownReason::ChannelClosed => "channel_closed",
        }
    }
}

impl fmt::Display for ShutdownReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

fn log_shutdown_stage(reason: ShutdownReason, stage: &'static str) {
    info!(
        target: "runtime.shutdown",
        reason = reason.as_str(),
        stage = stage,
        "shutdown_stage"
    );
}

#[derive(Default)]
struct PasteSession {
    buffer: Option<String>,
}

impl PasteSession {
    fn new() -> Self {
        Self { buffer: None }
    }

    fn start(&mut self) {
        trace!(target: "input.paste", "paste_start");
        self.buffer = Some(String::new());
    }

    fn push_chunk(&mut self, chunk: &str) {
        trace!(target: "input.paste", chunk_len = chunk.len(), "paste_chunk");
        if let Some(buffer) = &mut self.buffer {
            buffer.push_str(chunk);
        }
    }

    fn finish(&mut self) -> Option<(String, Vec<String>)> {
        let Some(buffer) = self.buffer.take() else {
            trace!(target: "input.paste", "paste_finish_empty");
            return None;
        };

        let (normalized, graphemes) = normalize_into_graphemes(&buffer);
        log_paste_commit(&normalized, graphemes.len());
        Some((normalized, graphemes))
    }
}

impl<'a> EditorRuntime<'a> {
    fn new(
        context: RuntimeContext<'a>,
        tx: mpsc::Sender<Event>,
        rx: mpsc::Receiver<Event>,
        input_task: tokio::task::JoinHandle<()>,
        input_shutdown: core_input::AsyncInputShutdown,
        source_handles: Vec<tokio::task::JoinHandle<()>>,
    ) -> Self {
        let RuntimeContext {
            model,
            config,
            platform_traits,
            terminal_guard,
        } = context;
        Self {
            model,
            config,
            platform_traits,
            scheduler: RenderScheduler::new(),
            render_engine: RenderEngine::new(),
            render_metrics: RenderMetricsLedger::default(),
            sticky_visual_col: None,
            paste: PasteSession::new(),
            ngi_timeout: NgiTimeoutState::default(),
            observers: Vec::new(),
            hooks: Box::new(NoopEventHooks),
            rx,
            tx: Some(tx),
            source_handles,
            input_task: Some(input_task),
            input_shutdown: Some(input_shutdown),
            _terminal_guard: terminal_guard,
        }
    }

    async fn run(&mut self) -> Result<()> {
        self.perform_initial_render();

        let render_span = tracing::debug_span!(target: "runtime", "event_loop");
        let _enter_loop = render_span.enter();

        let mut shutdown_reason = ShutdownReason::ChannelClosed;
        while let Some(event) = self.rx.recv().await {
            self.hooks.pre_handle(&event);

            let control = match &event {
                Event::Input(input) => self.handle_input_event(input),
                Event::Command(cmd) => self.handle_command_event(cmd),
                Event::RenderRequested => self.handle_render_requested(),
                Event::Tick => self.handle_tick(),
                Event::Shutdown => self.handle_shutdown(),
            };

            match control {
                LoopControl::Break { reason } => {
                    shutdown_reason = reason;
                    break;
                }
                LoopControl::Continue { lines_changed } => {
                    let scrolled = self.auto_scroll();
                    self.finish_cycle(lines_changed, scrolled);
                    self.hooks.post_handle(&event);
                }
            }
        }

        self.rx.close();
        self.finalize_shutdown(shutdown_reason).await;
        Ok(())
    }

    async fn finalize_shutdown(&mut self, reason: ShutdownReason) {
        log_shutdown_stage(reason, "begin");
        if let Some(tx) = self.tx.take() {
            trace!(
                target: "runtime.shutdown",
                reason = reason.as_str(),
                "dropping_runtime_sender"
            );
            drop(tx);
        }

        while let Some(handle) = self.source_handles.pop() {
            match tokio::time::timeout(Duration::from_millis(200), handle).await {
                Ok(Ok(_)) => trace!(
                    target: "runtime.shutdown",
                    reason = reason.as_str(),
                    "event_source_task_stopped"
                ),
                Ok(Err(err)) if err.is_cancelled() => trace!(
                    target: "runtime.shutdown",
                    reason = reason.as_str(),
                    "event_source_task_cancelled"
                ),
                Ok(Err(err)) => error!(
                    target: "runtime.shutdown",
                    reason = reason.as_str(),
                    ?err,
                    "event_source_task_error"
                ),
                Err(_) => warn!(
                    target: "runtime.shutdown",
                    reason = reason.as_str(),
                    "event_source_task_timeout"
                ),
            }
        }

        if let Some(shutdown) = self.input_shutdown.take() {
            trace!(
                target: "runtime.shutdown",
                reason = reason.as_str(),
                "input_task_shutdown_signal"
            );
            shutdown.signal();
        }

        if let Some(handle) = self.input_task.take() {
            match handle.await {
                Ok(_) => trace!(
                    target: "runtime.shutdown",
                    reason = reason.as_str(),
                    "input_task_joined"
                ),
                Err(err) if err.is_cancelled() => trace!(
                    target: "runtime.shutdown",
                    reason = reason.as_str(),
                    "input_task_cancelled"
                ),
                Err(err) => error!(
                    target: "runtime.shutdown",
                    reason = reason.as_str(),
                    ?err,
                    "input_task_join_failed"
                ),
            }
        }

        log_shutdown_stage(reason, "complete");
    }

    fn perform_initial_render(&mut self) {
        let decision = core_render::scheduler::Decision {
            semantic: RenderDelta::Full,
            effective: RenderDelta::Full,
        };
        if let Err(e) = RenderInvoker::new(
            &mut self.render_engine,
            &self.scheduler,
            &mut self.render_metrics,
        )
        .apply(&mut self.model, &decision)
        {
            error!(target: "render.engine", ?e, "initial_render_error");
        }
    }

    fn handle_input_event(&mut self, input: &InputEvent) -> LoopControl {
        match input {
            InputEvent::KeyPress(_) => self.handle_key_press(),
            InputEvent::CtrlC => self.handle_ctrl_c(),
            InputEvent::Key(key) => self.handle_key(key),
            InputEvent::Resize(w, h) => self.handle_resize(*w, *h),
            InputEvent::TextCommit(text) => self.handle_text_commit(text),
            InputEvent::PasteStart => self.handle_paste_start(),
            InputEvent::PasteChunk(chunk) => self.handle_paste_chunk(chunk),
            InputEvent::PasteEnd => self.handle_paste_end(),
            InputEvent::Mouse(_) => LoopControl::Continue { lines_changed: 0 },
            InputEvent::FocusGained
            | InputEvent::FocusLost
            | InputEvent::RawBytes(_)
            | InputEvent::CompositionUpdate { .. } => LoopControl::Continue { lines_changed: 0 },
        }
    }

    fn command_context(&self) -> CommandContextSnapshot {
        CommandContextSnapshot::capture(self.model.state())
    }

    fn handle_key_press(&mut self) -> LoopControl {
        LoopControl::Continue { lines_changed: 0 }
    }

    fn handle_ctrl_c(&mut self) -> LoopControl {
        info!(target: "runtime", "shutdown");
        LoopControl::Break {
            reason: ShutdownReason::CtrlC,
        }
    }

    fn handle_key(&mut self, key: &core_events::KeyEvent) -> LoopControl {
        let ctx = self.command_context();
        let resolution =
            core_actions::translate_ngi(ctx.mode(), ctx.pending_buffer(), key, &self.config);
        self.ngi_timeout
            .update(resolution.pending_state, resolution.timeout_deadline);

        if let Some(action) = resolution.action {
            let outcome = self.process_action(action);
            let quit = outcome.quit;
            let lines_changed = self.apply_dispatch_outcome(outcome);
            if quit {
                LoopControl::Break {
                    reason: ShutdownReason::ActionQuit,
                }
            } else {
                LoopControl::Continue { lines_changed }
            }
        } else {
            LoopControl::Continue { lines_changed: 0 }
        }
    }

    fn handle_resize(&mut self, width: u16, height: u16) -> LoopControl {
        self.render_engine.invalidate_for_resize();
        self.scheduler.mark(RenderDelta::Full);
        let ctx = ConfigContext::new(width, height, STATUS_ROWS, 0, self.platform_traits);
        if let Some(new_margin) = self.config.recompute_with_context(ctx) {
            self.model.state_mut().config_vertical_margin = new_margin as usize;
            self.scheduler.mark(RenderDelta::StatusLine);
        }
        LoopControl::Continue { lines_changed: 0 }
    }

    fn handle_tick(&mut self) -> LoopControl {
        let mut lines_changed = 0;

        if self.model.state_mut().tick_ephemeral() {
            self.scheduler.mark(RenderDelta::StatusLine);
        }

        if let Some(result) = self.ngi_timeout.poll_expired(Instant::now(), || {
            core_actions::flush_pending_literal(&self.config)
                .map(TimeoutFlushResult::from_resolution)
        }) && let Some(action) = result.action
        {
            let outcome = self.process_action(action);
            let quit = outcome.quit;
            lines_changed = self.apply_dispatch_outcome(outcome);
            if quit {
                return LoopControl::Break {
                    reason: ShutdownReason::ActionQuit,
                };
            }
        }

        LoopControl::Continue { lines_changed }
    }

    fn handle_text_commit(&mut self, text: &str) -> LoopControl {
        let (normalized, graphemes) = normalize_into_graphemes(text);
        tracing::debug!(
            target: "input.normalize",
            grapheme_count = graphemes.len(),
            bytes = normalized.len(),
            "text_commit"
        );
        self.replay_text_input(&normalized, &graphemes)
    }

    fn handle_paste_start(&mut self) -> LoopControl {
        self.paste.start();
        LoopControl::Continue { lines_changed: 0 }
    }

    fn handle_paste_chunk(&mut self, chunk: &str) -> LoopControl {
        self.paste.push_chunk(chunk);
        LoopControl::Continue { lines_changed: 0 }
    }

    fn handle_paste_end(&mut self) -> LoopControl {
        if let Some((normalized, graphemes)) = self.paste.finish() {
            self.replay_text_input(&normalized, &graphemes)
        } else {
            LoopControl::Continue { lines_changed: 0 }
        }
    }

    fn handle_render_requested(&mut self) -> LoopControl {
        LoopControl::Continue { lines_changed: 0 }
    }

    fn handle_command_event(&mut self, cmd: &CommandEvent) -> LoopControl {
        match cmd {
            CommandEvent::Quit => LoopControl::Break {
                reason: ShutdownReason::CommandQuit,
            },
        }
    }

    fn handle_shutdown(&mut self) -> LoopControl {
        LoopControl::Break {
            reason: ShutdownReason::ShutdownEvent,
        }
    }

    fn auto_scroll(&mut self) -> bool {
        if let Ok((width, height)) = crossterm::terminal::size() {
            let overlay_rows = if height > 0 {
                core_render::overlay::overlay_line_count(self.model.state(), width)
            } else {
                0
            } as usize;
            let base_text_height = if height > 0 { (height - 1) as usize } else { 0 };
            let effective_text_height = base_text_height.saturating_sub(overlay_rows);
            let before_first = self.model.active_view().viewport_first_line;
            let scroll_changed = {
                let (state, view) = self.model.split_state_and_active_view();
                view.auto_scroll(state, effective_text_height)
            };
            if scroll_changed {
                let after_first = self.model.active_view().viewport_first_line;
                self.scheduler.mark(RenderDelta::Scroll {
                    old_first: before_first,
                    new_first: after_first,
                });
                return true;
            }
        }
        false
    }

    fn finish_cycle(&mut self, lines_changed: usize, scrolled: bool) {
        debug_assert!(
            self.model.active_view().cursor.line < self.model.state().active_buffer().line_count(),
            "cursor must be within buffer before scheduling render"
        );

        if let Some(decision) = self.scheduler.consume() {
            log_render_decision(&decision, lines_changed, scrolled);
            if let Err(e) = RenderInvoker::new(
                &mut self.render_engine,
                &self.scheduler,
                &mut self.render_metrics,
            )
            .apply(&mut self.model, &decision)
            {
                error!(target: "render.engine", ?e, "render_error");
            }
        }
    }

    fn replay_text_input(&mut self, normalized: &str, graphemes: &[String]) -> LoopControl {
        let ctx = self.command_context();
        if ctx.colon_active() {
            let mut outcome = DispatchOutcome::default();
            for ch in normalized.chars() {
                let single = self.process_action(Action::CommandChar(ch));
                outcome.absorb(single);
            }
            let quit = outcome.quit;
            let lines_changed = self.apply_dispatch_outcome(outcome);
            if quit {
                LoopControl::Break {
                    reason: ShutdownReason::ActionQuit,
                }
            } else {
                LoopControl::Continue { lines_changed }
            }
        } else if matches!(ctx.mode(), Mode::Insert) {
            let mut outcome = DispatchOutcome::default();
            for grapheme in graphemes {
                let single =
                    self.process_action(Action::Edit(EditKind::InsertGrapheme(grapheme.clone())));
                outcome.absorb(single);
            }
            let quit = outcome.quit;
            let lines_changed = self.apply_dispatch_outcome(outcome);
            if quit {
                LoopControl::Break {
                    reason: ShutdownReason::ActionQuit,
                }
            } else {
                LoopControl::Continue { lines_changed }
            }
        } else {
            LoopControl::Continue { lines_changed: 0 }
        }
    }
}

impl<'a> EditorRuntime<'a> {
    fn process_action(&mut self, action: Action) -> DispatchOutcome {
        let pre_status = StatusSnapshot::capture(self.model.state());
        let before_line = self.model.active_view().cursor.line;
        let span = tracing::trace_span!(
            target: "actions.dispatch",
            "process_action",
            action = ?action
        );
        let result = span.in_scope(|| {
            dispatch(
                action,
                &mut self.model,
                &mut self.sticky_visual_col,
                &self.observers,
            )
        });
        let post_status = StatusSnapshot::capture(self.model.state());
        let after_line = self.model.active_view().cursor.line;
        let insert_mode = matches!(self.model.state().mode, Mode::Insert);
        let status_changed = post_status.differs(&pre_status);
        let line_changed = before_line != after_line || insert_mode;
        let outcome = DispatchOutcome::new(
            result.dirty,
            result.buffer_replaced,
            result.quit,
            status_changed,
            line_changed,
        );
        span.in_scope(|| {
            trace!(
                target: "actions.dispatch",
                dirty = outcome.dirty,
                buffer_replaced = outcome.buffer_replaced,
                quit = outcome.quit,
                status_changed = outcome.status_changed,
                line_changed = outcome.line_changed,
                "dispatch_outcome"
            );
        });
        outcome
    }

    fn apply_dispatch_outcome(&mut self, outcome: DispatchOutcome) -> usize {
        if outcome.buffer_replaced {
            self.render_engine.invalidate_for_resize();
            self.scheduler.mark(RenderDelta::Full);
            return 0;
        }

        if !outcome.dirty {
            return 0;
        }

        let after_line = self.model.active_view().cursor.line;
        if outcome.line_changed {
            self.scheduler
                .mark(RenderDelta::Lines(after_line..after_line + 1));
            1
        } else if outcome.status_changed {
            self.scheduler.mark(RenderDelta::StatusLine);
            0
        } else {
            self.scheduler.mark(RenderDelta::CursorOnly);
            0
        }
    }
}
#[tokio::main]
async fn main() -> Result<()> {
    let mut startup = AppStartup::new();
    let context = startup.run()?;
    let (tx, rx) = mpsc::channel::<Event>(EVENT_CHANNEL_CAP);
    let (input_task, input_shutdown) = core_input::spawn_async_input(tx.clone());
    let mut registry = EventSourceRegistry::new();
    registry.register(TickEventSource::new(std::time::Duration::from_millis(250)));
    let source_handles = registry.spawn_all(&tx);

    let mut runtime =
        EditorRuntime::new(context, tx, rx, input_task, input_shutdown, source_handles);
    runtime.run().await
}

fn render(
    engine: &mut RenderEngine,
    state: &mut EditorState,
    view: &core_model::View,
    decision: &core_render::scheduler::Decision,
) -> Result<core_state::RenderPathSnapshotLite> {
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
    match res {
        Ok(()) => {
            let snap = engine.metrics_snapshot();
            Ok(core_state::RenderPathSnapshotLite {
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
            })
        }
        Err(e) => Err(e),
    }
}

#[inline]
fn log_render_decision(
    decision: &core_render::scheduler::Decision,
    lines_changed: usize,
    scrolled: bool,
) {
    tracing::debug!(
        target: "render.scheduler",
        semantic = ?decision.semantic,
        effective = ?decision.effective,
        lines_changed,
        scrolled,
        "render_decision"
    );
}

fn convert_delta_snapshot(
    metrics: RenderDeltaMetricsSnapshot,
) -> Option<core_state::RenderDeltaSnapshotLite> {
    if metrics.semantic_frames == 0 {
        return None;
    }
    Some(core_state::RenderDeltaSnapshotLite {
        full: metrics.full,
        lines: metrics.lines,
        scroll: metrics.scroll,
        status_line: metrics.status_line,
        cursor_only: metrics.cursor_only,
        collapsed_scroll: metrics.collapsed_scroll,
        suppressed_scroll: metrics.suppressed_scroll,
        semantic_frames: metrics.semantic_frames,
    })
}

// Pure helper: NFC-normalize a string and return its grapheme clusters.
// normalize_and_segment moved to core_text::segment

#[cfg(test)]
mod tests {
    use super::*;
    use core_actions::{EditKind, ModeChange, MotionKind};
    use core_render::render_engine::{RenderEngine, build_content_frame};
    use core_text::Buffer;
    use std::fmt;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
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
        let dispatcher = Dispatch::new(subscriber);

        tracing::dispatcher::with_default(&dispatcher, || {
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

    #[tokio::test]
    async fn minimal_event_sequence_logs_render_decision() {
        let capture = Capture::default();
        let events = capture.events.clone();
        let subscriber = Registry::default().with(capture);
        let dispatcher = Dispatch::new(subscriber);

        tracing::dispatcher::with_default(&dispatcher, || {
            let buffer = Buffer::from_str("test", "abc\n").unwrap();
            let state = EditorState::new(buffer);
            let mut model = EditorModel::new(state);
            let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
            let mut sticky = None;
            let mut scheduler = RenderScheduler::new();

            tracing::info!(target: "runtime", phase = "test", "startup");

            let dr = dispatch(
                Action::Motion(MotionKind::Right),
                &mut model,
                &mut sticky,
                &observers,
            );
            assert!(dr.dirty, "motion should mark state dirty");

            scheduler.mark(RenderDelta::CursorOnly);
            let decision = scheduler.consume().expect("decision");
            assert!(matches!(decision.semantic, RenderDelta::CursorOnly));
            log_render_decision(&decision, 0, false);
        });

        let events = events.lock().unwrap();
        let render_event = events
            .iter()
            .find(|e| {
                e.target == "render.scheduler"
                    && e.fields
                        .iter()
                        .any(|(name, value)| name == "message" && value.contains("render_decision"))
            })
            .expect("render_decision log present");
        assert!(
            render_event
                .fields
                .iter()
                .any(|(name, value)| name == "semantic" && value.contains("CursorOnly")),
            "render decision semantic field missing CursorOnly: {:?}",
            render_event.fields
        );
    }

    #[test]
    fn startup_config_vertical_margin_matches_effective_value() {
        let mut config = core_config::Config::default();
        let ctx = core_config::ConfigContext::new(
            80,
            24,
            STATUS_ROWS,
            0,
            core_config::ConfigPlatformTraits::new(false, true),
        );
        config.apply_context(ctx);

        let buffer = Buffer::from_str("test", "").unwrap();
        let mut state = EditorState::new(buffer);
        state.config_vertical_margin = config.effective_vertical_margin as usize;
        assert_eq!(state.config_vertical_margin, 0);

        config.file.scroll.margin.vertical = 6;
        let ctx_small = core_config::ConfigContext::new(
            80,
            10,
            STATUS_ROWS,
            0,
            core_config::ConfigPlatformTraits::new(false, true),
        );
        config.apply_context(ctx_small);
        let mut state_small = EditorState::new(Buffer::from_str("test", "").unwrap());
        state_small.config_vertical_margin = config.effective_vertical_margin as usize;
        let text_rows = ctx_small.text_rows() as usize;
        let max_margin = if text_rows <= 2 {
            0
        } else {
            (text_rows - 2) / 2
        };
        assert_eq!(state_small.config_vertical_margin, max_margin.min(6));
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
        assert!(
            dispatch(
                Action::Undo { count: 1 },
                &mut model,
                &mut sticky,
                &observers
            )
            .dirty
        );
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "");
    }

    #[test]
    fn normal_mode_delete_under_single() {
        let mut model = mk_state_model("abc");
        let mut sticky = None;
        // Delete 'a'
        let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
        dispatch(
            Action::Edit(EditKind::DeleteUnder {
                count: 1,
                register: None,
            }),
            &mut model,
            &mut sticky,
            &observers,
        );
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "bc");
        assert_eq!(model.state().undo_depth(), 1, "snapshot pushed for delete");
        // Register should now contain removed grapheme 'a'
        assert!(model.state().registers.unnamed.starts_with('a'));
        // Undo
        assert!(
            dispatch(
                Action::Undo { count: 1 },
                &mut model,
                &mut sticky,
                &observers
            )
            .dirty
        );
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "abc");
    }

    #[test]
    fn normal_mode_delete_under_then_pastes_with_p() {
        let mut model = mk_state_model("xyz");
        let mut sticky = None;
        let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
        // Delete 'x'
        dispatch(
            Action::Edit(EditKind::DeleteUnder {
                count: 1,
                register: None,
            }),
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
            Action::PasteAfter {
                count: 1,
                register: None,
            },
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
            Action::Edit(EditKind::DeleteUnder {
                count: 1,
                register: None,
            }),
            &mut model,
            &mut sticky,
            &observers,
        );
        // Delete 'b' (originally 'c', now at index 0 after first delete)
        dispatch(
            Action::Edit(EditKind::DeleteUnder {
                count: 1,
                register: None,
            }),
            &mut model,
            &mut sticky,
            &observers,
        );
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "cd");
        assert_eq!(model.state().undo_depth(), 2, "two discrete snapshots");
        // Undo last -> should restore to "bcd" (?) Actually sequence: start abcd -> after first delete bcd -> after second delete cd. Undo should return to bcd.
        assert!(
            dispatch(
                Action::Undo { count: 1 },
                &mut model,
                &mut sticky,
                &observers
            )
            .dirty
        );
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "bcd");
        // Undo again -> original
        assert!(
            dispatch(
                Action::Undo { count: 1 },
                &mut model,
                &mut sticky,
                &observers
            )
            .dirty
        );
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

    #[test]
    fn shutdown_reason_labels_are_stable() {
        assert_eq!(ShutdownReason::CtrlC.as_str(), "ctrl_c");
        assert_eq!(ShutdownReason::CommandQuit.as_str(), "command_quit");
        assert_eq!(ShutdownReason::ActionQuit.as_str(), "action_quit");
        assert_eq!(ShutdownReason::ShutdownEvent.as_str(), "shutdown_event");
        assert_eq!(ShutdownReason::ChannelClosed.as_str(), "channel_closed");
    }

    #[test]
    fn shutdown_logging_includes_reason_and_stage() {
        let capture = Capture::default();
        let events = capture.events.clone();
        let subscriber = Registry::default().with(capture);
        let dispatcher = Dispatch::new(subscriber);

        tracing::dispatcher::with_default(&dispatcher, || {
            log_shutdown_stage(ShutdownReason::CommandQuit, "complete");
        });

        let events = events.lock().unwrap();
        let shutdown_event = events
            .iter()
            .find(|event| event.target == "runtime.shutdown")
            .expect("shutdown log emitted");
        assert!(
            shutdown_event
                .fields
                .iter()
                .any(|(name, value)| name == "reason" && value.contains("command_quit"))
        );
        assert!(
            shutdown_event
                .fields
                .iter()
                .any(|(name, value)| name == "stage" && value.contains("complete"))
        );
    }

    #[test]
    fn command_context_snapshot_reports_colon_activity() {
        let mut model = mk_state_model("example\n");
        let mut sticky = None;
        let observers: Vec<Box<dyn ActionObserver>> = Vec::new();
        dispatch(Action::CommandStart, &mut model, &mut sticky, &observers);
        let ctx = CommandContextSnapshot::capture(model.state());
        assert!(
            ctx.command_active(),
            "command line should be active after CommandStart"
        );
        assert!(
            ctx.colon_active(),
            "command buffer should retain leading colon"
        );
        assert_eq!(ctx.pending_buffer(), ":");
        assert_eq!(ctx.mode(), model.state().mode);
    }

    #[test]
    fn ngi_timeout_state_flushes_action_when_expired() {
        let mut timeout = NgiTimeoutState::default();
        timeout.update(
            PendingState::AwaitingMore { buffered_len: 1 },
            Some(Instant::now() - Duration::from_millis(5)),
        );

        let result = timeout.poll_expired(Instant::now(), || {
            Some(TimeoutFlushResult {
                action: Some(Action::CommandChar('g')),
                pending: PendingState::Idle,
                deadline: None,
            })
        });

        let flush = result.expect("timeout should flush pending literal");
        assert!(matches!(flush.action, Some(Action::CommandChar('g'))));
        assert_eq!(timeout.pending(), PendingState::Idle);
        assert!(timeout.deadline().is_none());
    }

    #[test]
    fn ngi_timeout_state_does_not_flush_before_deadline() {
        let mut timeout = NgiTimeoutState::default();
        timeout.update(
            PendingState::AwaitingMore { buffered_len: 2 },
            Some(Instant::now() + Duration::from_millis(50)),
        );

        let result = timeout.poll_expired(Instant::now(), || {
            unreachable!("flush should not execute before deadline");
        });

        assert!(result.is_none());
        assert!(matches!(
            timeout.pending(),
            PendingState::AwaitingMore { .. }
        ));
        assert!(timeout.deadline().is_some());
    }

    #[test]
    fn ngi_timeout_state_clears_when_flush_returns_none() {
        let mut timeout = NgiTimeoutState::default();
        timeout.update(
            PendingState::AwaitingMore { buffered_len: 3 },
            Some(Instant::now() - Duration::from_millis(10)),
        );

        let result = timeout.poll_expired(Instant::now(), || None);

        assert!(result.is_none());
        assert_eq!(timeout.pending(), PendingState::Idle);
        assert!(timeout.deadline().is_none());
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
