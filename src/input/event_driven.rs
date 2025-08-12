use crate::core::editor::Editor;
use crate::input::events::*;
use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use log::{error, info, warn};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

/// Event-driven editor with true async architecture
pub struct EventDrivenEditor {
    /// Shared editor state
    editor: Arc<Mutex<Editor>>,
    /// Event bus for communication between threads
    event_sender: mpsc::Sender<EditorEvent>,
    event_receiver: mpsc::Receiver<EditorEvent>,
    /// Control channels for thread management
    shutdown_sender: mpsc::Sender<()>,
    /// Thread handles for cleanup
    thread_handles: Vec<thread::JoinHandle<()>>,
    /// Track last render state to minimize unnecessary redraws
    last_render_state: Arc<Mutex<RenderState>>,
}

// Centralized timing constants for event loop cadence
// Single tick used for both main event recv timeout and input polling
const EVENT_TICK_MS: u64 = 16;

#[derive(Debug, Clone, PartialEq)]
struct RenderState {
    mode: crate::core::mode::Mode,
    cursor_position: Option<(usize, usize)>,
    buffer_id: Option<usize>,
    command_line: String,
    status_message: String,
    // Whether a macro is currently recording (used to trigger redraw for statusline REC indicator)
    macro_recording_active: bool,
    needs_full_redraw: bool,
    // Track command completion state so UI can redraw when it changes
    completion_active: bool,
    completion_matches_len: usize,
    completion_selected_index: usize,
}

impl EventDrivenEditor {
    /// Create a new event-driven editor with background threads
    pub fn new(editor: Editor) -> Self {
        let editor = Arc::new(Mutex::new(editor));
        let (event_sender, event_receiver) = mpsc::channel();
        let (shutdown_sender, shutdown_receiver) = mpsc::channel();

        let mut thread_handles = Vec::new();

        // Start input event thread
        let input_thread =
            Self::spawn_input_thread(editor.clone(), event_sender.clone(), shutdown_receiver);
        thread_handles.push(input_thread);

        // Start config watcher thread
        let config_thread = Self::spawn_config_watcher_thread(editor.clone(), event_sender.clone());
        thread_handles.push(config_thread);

        // Start syntax results dispatcher thread (async highlighter results -> events)
        let syntax_thread = Self::spawn_syntax_results_thread(editor.clone(), event_sender.clone());
        thread_handles.push(syntax_thread);

        // Start rendering thread
        let render_thread = Self::spawn_render_thread(editor.clone(), event_sender.clone());
        thread_handles.push(render_thread);

        Self {
            editor,
            event_sender,
            event_receiver,
            shutdown_sender,
            thread_handles,
            last_render_state: Arc::new(Mutex::new(RenderState {
                mode: crate::core::mode::Mode::Normal,
                cursor_position: None,
                buffer_id: None,
                command_line: String::new(),
                status_message: String::new(),
                macro_recording_active: false,
                needs_full_redraw: true, // Force initial full redraw
                completion_active: false,
                completion_matches_len: 0,
                completion_selected_index: 0,
            })),
        }
    }

    /// Main event loop - processes events from all background threads
    pub fn run(&mut self) -> Result<()> {
        info!("Starting truly event-driven editor");

        // Send initial events
        self.send_initial_events()?;

        // Main event processing loop
        loop {
            match self.event_receiver.recv() {
                Ok(event) => {
                    let should_quit = self.process_event(event)?;
                    if should_quit {
                        break;
                    }
                }
                Err(_disconnected) => {
                    warn!("Event channel disconnected");
                    break;
                }
            }
        }

        self.shutdown()?;
        Ok(())
    }

    /// Capture a condensed render state snapshot from the editor for change detection
    fn snapshot(editor: &Editor) -> RenderState {
        RenderState {
            mode: editor.mode(),
            cursor_position: editor
                .current_buffer()
                .map(|b| (b.cursor.row, b.cursor.col)),
            buffer_id: editor.current_buffer_id,
            command_line: editor.command_line().to_string(),
            status_message: editor.status_message().to_string(),
            macro_recording_active: editor.is_macro_recording(),
            needs_full_redraw: false, // managed separately
            completion_active: editor.is_completion_active(),
            completion_matches_len: editor.completion_matches_len(),
            completion_selected_index: editor.completion_selected_index(),
        }
    }

    /// Send initial setup events
    fn send_initial_events(&self) -> Result<()> {
        // Create initial buffer if needed
        if let Ok(mut editor) = self.editor.lock()
            && editor.current_buffer().is_none()
            && let Err(e) = editor.create_buffer(None)
        {
            warn!("Failed to create initial buffer: {}", e);
        }

        // Request initial render
        self.event_sender
            .send(EditorEvent::UI(UIEvent::RedrawRequest))?;

        Ok(())
    }

    /// Process a single event and return whether to quit
    fn process_event(&self, event: EditorEvent) -> Result<bool> {
        // First, delegate to specific handler
        let handled_quit = match event {
            EditorEvent::Input(input_event) => self.handle_input_event(input_event)?,
            EditorEvent::UI(ui_event) => self.handle_ui_event(ui_event)?,
            EditorEvent::Buffer(buffer_event) => self.handle_buffer_event(buffer_event)?,
            EditorEvent::Window(window_event) => self.handle_window_event(window_event)?,
            EditorEvent::Config(config_event) => self.handle_config_event(config_event)?,
            EditorEvent::Search(search_event) => self.handle_search_event(search_event)?,
            EditorEvent::System(system_event) => self.handle_system_event(system_event)?,
            EditorEvent::Plugin(_) => false, // Future implementation
            EditorEvent::LSP(_) => false,    // Future implementation
            EditorEvent::Macro(macro_event) => self.handle_macro_event(macro_event)?,
        };

        // Then, as a belt-and-suspenders, check the editor's should_quit flag
        if handled_quit {
            return Ok(true);
        }
        if let Ok(editor) = self.editor.lock()
            && editor.should_quit()
        {
            return Ok(true);
        }
        Ok(false)
    }

    /// Handle input events
    fn handle_input_event(&self, event: InputEvent) -> Result<bool> {
        match event {
            InputEvent::KeyPress(key_event) => {
                if let Ok(mut editor) = self.editor.lock() {
                    // Snapshot before
                    let before = Self::snapshot(&editor);

                    // Process the key event
                    editor.handle_key_event(key_event)?;

                    // Snapshot after
                    let after = Self::snapshot(&editor);

                    // Check what actually changed to decide if we need a redraw
                    let needs_redraw = before != after
                        || editor
                            .needs_syntax_refresh
                            .load(std::sync::atomic::Ordering::Relaxed)
                        || editor
                            .needs_redraw
                            .load(std::sync::atomic::Ordering::Relaxed);

                    // Only request render if something actually changed
                    if needs_redraw {
                        let _ = self
                            .event_sender
                            .send(EditorEvent::UI(UIEvent::RedrawRequest));
                    }

                    Ok(editor.should_quit())
                } else {
                    Ok(false)
                }
            }
            InputEvent::Command(cmd) => {
                // Process command
                if let Ok(mut editor) = self.editor.lock() {
                    // Handle command through existing command system
                    editor.set_command_line(cmd);
                    let _ = self
                        .event_sender
                        .send(EditorEvent::UI(UIEvent::RedrawRequest));
                }
                Ok(false)
            }
            InputEvent::ModeChange { from: _, to } => {
                if let Ok(mut editor) = self.editor.lock() {
                    editor.set_mode(to);
                    let _ = self
                        .event_sender
                        .send(EditorEvent::UI(UIEvent::RedrawRequest));
                }
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    /// Handle UI events
    fn handle_ui_event(&self, event: UIEvent) -> Result<bool> {
        match event {
            UIEvent::RedrawRequest => {
                if let (Ok(mut editor), Ok(mut last_state)) =
                    (self.editor.try_lock(), self.last_render_state.try_lock())
                {
                    // Snapshot current state and compare with last snapshot
                    let current = Self::snapshot(&editor);
                    // Compare ignoring the last_state.needs_full_redraw flag
                    let mut last_cmp = last_state.clone();
                    last_cmp.needs_full_redraw = false;
                    let needs_redraw = last_cmp != current
                        || last_state.needs_full_redraw
                        || editor
                            .needs_syntax_refresh
                            .load(std::sync::atomic::Ordering::Relaxed)
                        || editor
                            .needs_redraw
                            .load(std::sync::atomic::Ordering::Relaxed);

                    if needs_redraw {
                        if let Err(e) = editor.render() {
                            error!("Render failed: {}", e);
                        } else {
                            // Update our cached state
                            *last_state = current;
                            last_state.needs_full_redraw = false;

                            // Reset refresh flags
                            editor
                                .needs_syntax_refresh
                                .store(false, std::sync::atomic::Ordering::Relaxed);
                            editor
                                .needs_redraw
                                .store(false, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                }
                Ok(false)
            }
            UIEvent::Resize { width, height } => {
                if let Ok(mut editor) = self.editor.lock() {
                    editor.window_manager.resize_terminal(width, height);

                    // Force a full redraw after resize
                    if let Ok(mut last_state) = self.last_render_state.try_lock() {
                        last_state.needs_full_redraw = true;
                    }

                    let _ = self
                        .event_sender
                        .send(EditorEvent::UI(UIEvent::RedrawRequest));
                }
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    /// Handle buffer events
    fn handle_buffer_event(&self, event: BufferEvent) -> Result<bool> {
        if let Ok(mut editor) = self.editor.lock() {
            match event {
                BufferEvent::Created { buffer_id, path } => {
                    let name = path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "[No Name]".to_string());
                    editor.set_status_message(format!("Buffer {} created ({})", buffer_id, name));
                }
                BufferEvent::Opened { buffer_id, path } => {
                    editor.set_status_message(format!(
                        "Opened '{}' in buffer {}",
                        path.to_string_lossy(),
                        buffer_id
                    ));
                }
                BufferEvent::Modified { buffer_id } => {
                    editor.set_status_message(format!("Buffer {} modified", buffer_id));
                }
                BufferEvent::Saved { buffer_id, path } => {
                    editor.set_status_message(format!(
                        "Buffer {} saved to {}",
                        buffer_id,
                        path.to_string_lossy()
                    ));
                }
                BufferEvent::Closed { buffer_id } => {
                    editor.set_status_message(format!("Buffer {} closed", buffer_id));
                }
                BufferEvent::ContentChanged { .. }
                | BufferEvent::CursorMoved { .. }
                | BufferEvent::SelectionChanged { .. }
                | BufferEvent::SyntaxHighlighted { .. } => {
                    // For now, just request a redraw
                }
            }

            // Request redraw after handling buffer events
            let _ = self
                .event_sender
                .send(EditorEvent::UI(UIEvent::RedrawRequest));
        }
        Ok(false)
    }

    /// Handle window events
    fn handle_window_event(&self, event: WindowEvent) -> Result<bool> {
        if let Ok(mut editor) = self.editor.lock() {
            match event {
                WindowEvent::Created { window_id } => {
                    editor.set_status_message(format!("Window {} created", window_id));
                }
                WindowEvent::Closed { window_id } => {
                    editor.set_status_message(format!("Window {} closed", window_id));
                }
                WindowEvent::Split {
                    parent_id,
                    new_window_id,
                    direction,
                } => {
                    editor.set_status_message(format!(
                        "Window {} split {:?} -> {}",
                        parent_id, direction, new_window_id
                    ));
                }
                WindowEvent::FocusChanged {
                    old_window_id: _,
                    new_window_id,
                } => {
                    // Update focus in window manager and sync current buffer from focused window
                    editor.window_manager.set_current_window(new_window_id);
                    if let Some(win) = editor.window_manager.current_window() {
                        editor.current_buffer_id = win.buffer_id;
                    }
                }
                WindowEvent::Resized {
                    window_id,
                    width,
                    height,
                } => {
                    editor.set_status_message(format!(
                        "Window {} resized to {}x{}",
                        window_id, width, height
                    ));
                }
            }
            let _ = self
                .event_sender
                .send(EditorEvent::UI(UIEvent::RedrawRequest));
        }
        Ok(false)
    }

    /// Handle config events
    fn handle_config_event(&self, event: ConfigEvent) -> Result<bool> {
        match event {
            ConfigEvent::EditorConfigChanged => {
                if let Ok(mut editor) = self.editor.lock() {
                    editor.reload_editor_config();
                }
                // Force a full redraw for config changes by setting the flag
                if let Ok(mut render_state) = self.last_render_state.try_lock() {
                    render_state.needs_full_redraw = true;
                }
                let _ = self
                    .event_sender
                    .send(EditorEvent::UI(UIEvent::RedrawRequest));
            }
            ConfigEvent::KeymapConfigChanged => {
                if let Ok(mut editor) = self.editor.lock() {
                    editor.reload_keymap_config();
                }
                // Force a full redraw for keymap changes
                if let Ok(mut render_state) = self.last_render_state.try_lock() {
                    render_state.needs_full_redraw = true;
                }
                let _ = self
                    .event_sender
                    .send(EditorEvent::UI(UIEvent::RedrawRequest));
            }
            ConfigEvent::ThemeConfigChanged => {
                if let Ok(mut editor) = self.editor.lock() {
                    editor.reload_ui_theme();
                }
                // Force a full redraw for theme changes
                if let Ok(mut render_state) = self.last_render_state.try_lock() {
                    render_state.needs_full_redraw = true;
                }
                let _ = self
                    .event_sender
                    .send(EditorEvent::UI(UIEvent::RedrawRequest));
            }
            _ => {}
        }
        Ok(false)
    }

    /// Handle search events
    fn handle_search_event(&self, event: SearchEvent) -> Result<bool> {
        if let Ok(mut editor) = self.editor.lock() {
            match event {
                SearchEvent::Started { pattern, is_regex } => {
                    editor.set_use_regex(is_regex);
                    editor.search(&pattern);
                }
                SearchEvent::ResultsFound(_results) => {
                    // Results are computed by editor.search; no-op for now.
                }
                SearchEvent::Navigate { direction } => match direction {
                    SearchDirection::Forward => {
                        editor.search_next();
                    }
                    SearchDirection::Backward => {
                        editor.search_previous();
                    }
                },
                SearchEvent::Cancelled => {
                    editor.clear_search();
                }
            }
            let _ = self
                .event_sender
                .send(EditorEvent::UI(UIEvent::RedrawRequest));
        }
        Ok(false)
    }

    /// Handle system events
    fn handle_system_event(&self, event: SystemEvent) -> Result<bool> {
        match event {
            SystemEvent::Quit => Ok(true),
            SystemEvent::ForceQuit => Ok(true),
            _ => Ok(false),
        }
    }

    /// Spawn input handling thread
    fn spawn_input_thread(
        _editor: Arc<Mutex<Editor>>,
        sender: mpsc::Sender<EditorEvent>,
        shutdown_receiver: mpsc::Receiver<()>,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            info!("Input thread started");

            loop {
                // Check for shutdown signal (non-blocking)
                if let Ok(()) = shutdown_receiver.try_recv() {
                    info!("Input thread shutting down");
                    break;
                }

                // Poll for terminal events
                match event::poll(Duration::from_millis(EVENT_TICK_MS)) {
                    Ok(true) => {
                        match event::read() {
                            Ok(Event::Key(key_event)) => {
                                // Only process key press events to avoid duplicates
                                if key_event.kind == KeyEventKind::Press
                                    && let Err(e) = sender
                                        .send(EditorEvent::Input(InputEvent::KeyPress(key_event)))
                                {
                                    error!("Failed to send key event: {}", e);
                                    break;
                                }
                            }
                            Ok(Event::Resize(width, height)) => {
                                if let Err(e) =
                                    sender.send(EditorEvent::UI(UIEvent::Resize { width, height }))
                                {
                                    error!("Failed to send resize event: {}", e);
                                    break;
                                }
                            }
                            Ok(_) => {} // Ignore other events
                            Err(e) => error!("Failed to read terminal event: {}", e),
                        }
                    }
                    Ok(false) => {} // No events available
                    Err(e) => error!("Failed to poll for events: {}", e),
                }
            }

            info!("Input thread finished");
        })
    }

    /// Spawn config watcher thread
    fn spawn_config_watcher_thread(
        editor: Arc<Mutex<Editor>>,
        sender: mpsc::Sender<EditorEvent>,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            info!("Config watcher thread started");

            // Obtain a clone of the receiver once, then block on it without
            // holding the editor lock.
            let rx = {
                if let Ok(ed) = editor.lock() {
                    ed.config_watcher.as_ref().map(|w| w.clone_receiver())
                } else {
                    None
                }
            };

            if rx.is_none() {
                info!("Config watcher thread finished (no watcher)");
                return;
            }

            let rx = rx.unwrap();

            loop {
                // Block waiting for watcher events; no polling/timeout
                let maybe_event = rx.lock().ok().and_then(|guard| guard.recv().ok());

                // If the watcher channel closed or watcher missing, exit
                let Some(first_event) = maybe_event else {
                    break;
                };

                // Build list with first event and any queued ones
                let mut changes = vec![first_event];
                if let Ok(ed) = editor.lock()
                    && let Some(ref watcher) = ed.config_watcher
                {
                    // Drain any additional pending events without waiting
                    changes.extend(watcher.check_for_changes());
                }

                for change in changes {
                    let event = match change {
                        crate::config::watcher::ConfigChangeEvent::EditorConfigChanged => {
                            EditorEvent::Config(ConfigEvent::EditorConfigChanged)
                        }
                        crate::config::watcher::ConfigChangeEvent::KeymapConfigChanged => {
                            EditorEvent::Config(ConfigEvent::KeymapConfigChanged)
                        }
                        crate::config::watcher::ConfigChangeEvent::ThemeConfigChanged => {
                            EditorEvent::Config(ConfigEvent::ThemeConfigChanged)
                        }
                    };

                    if let Err(e) = sender.send(event) {
                        error!("Failed to send config event: {}", e);
                        return;
                    }
                }
            }

            info!("Config watcher thread finished");
        })
    }

    // (syntax thread removed)
    /// Spawn syntax result dispatcher thread
    fn spawn_syntax_results_thread(
        editor: Arc<Mutex<Editor>>,
        sender: mpsc::Sender<EditorEvent>,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            info!("Syntax results dispatcher thread started");

            // Obtain a clone of the result receiver once
            let rx_opt = {
                if let Ok(ed) = editor.lock() {
                    ed.clone_syntax_result_receiver()
                } else {
                    None
                }
            };

            let Some(result_rx) = rx_opt else {
                info!("No async syntax highlighter; exiting syntax dispatcher");
                return;
            };

            while let Ok(result) = result_rx.recv() {
                // Forward as a buffer event; cache will be updated on handle
                // Update cache immediately and request UI redraw via event
                if let Ok(mut ed) = editor.lock() {
                    // Drop stale results (older version than editor's current)
                    let current_version = ed
                        .highlight_version
                        .load(std::sync::atomic::Ordering::Relaxed);
                    if result.version < current_version {
                        continue;
                    }
                    ed.apply_syntax_highlight_result(
                        result.buffer_id,
                        result.line_index,
                        result.highlights,
                    );
                }
                let _ = sender.send(EditorEvent::UI(UIEvent::RedrawRequest));
            }

            info!("Syntax results dispatcher thread finished");
        })
    }

    /// Spawn rendering thread
    fn spawn_render_thread(
        _editor: Arc<Mutex<Editor>>,
        _sender: mpsc::Sender<EditorEvent>,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            info!("Render thread started");

            // For now, rendering is handled synchronously in the main thread
            // In the future, this could handle background rendering optimizations

            // TODO: Implement background rendering optimizations
            // Currently no background work is done here.

            info!("Render thread finished");
        })
    }

    /// Shutdown all background threads
    fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down event-driven editor");

        // Drop config watcher to unblock the blocking watcher thread recv
        if let Ok(mut editor) = self.editor.lock() {
            // Stop async syntax worker so result channel closes and dispatcher exits
            editor.shutdown_async_syntax();
            // Stop config watcher
            editor.config_watcher = None;
        }

        // Signal all threads to shutdown
        let _ = self.shutdown_sender.send(());

        // Wait for all threads to finish
        for handle in self.thread_handles.drain(..) {
            if let Err(e) = handle.join() {
                error!("Failed to join thread: {:?}", e);
            }
        }

        info!("All background threads shut down");
        Ok(())
    }

    /// Handle macro events
    fn handle_macro_event(&self, event: crate::input::events::MacroEvent) -> Result<bool> {
        if let Ok(mut editor) = self.editor.lock() {
            match event {
                crate::input::events::MacroEvent::StartRecording(register) => {
                    if let Err(e) = editor.start_macro_recording(register) {
                        log::warn!("Failed to start macro recording: {}", e);
                    } else {
                        log::debug!("Started macro recording for register '{}'", register);
                        // Ensure UI updates immediately to show REC indicator
                        let _ = self
                            .event_sender
                            .send(EditorEvent::UI(UIEvent::RedrawRequest));
                    }
                }
                crate::input::events::MacroEvent::StopRecording => {
                    if editor.is_macro_recording() {
                        if let Err(e) = editor.stop_macro_recording() {
                            log::warn!("Failed to stop macro recording: {}", e);
                        } else {
                            log::debug!("Stopped macro recording");
                            // Ensure UI updates immediately to hide REC indicator
                            let _ = self
                                .event_sender
                                .send(EditorEvent::UI(UIEvent::RedrawRequest));
                        }
                    }
                }
                crate::input::events::MacroEvent::Execute { register, count: _ } => {
                    if let Err(e) = editor.play_macro(register) {
                        log::warn!("Failed to execute macro '{}': {}", register, e);
                    } else {
                        log::debug!("Executed macro '{}'", register);
                    }
                }
                crate::input::events::MacroEvent::RepeatLast { count: _ } => {
                    if let Err(e) = editor.play_last_macro() {
                        log::warn!("Failed to repeat last macro: {}", e);
                    } else {
                        log::debug!("Repeated last macro");
                    }
                }
            }
        }
        Ok(false)
    }

    /// Send an event to the event bus
    pub fn send_event(&self, event: EditorEvent) -> Result<()> {
        self.event_sender.send(event)?;
        Ok(())
    }
}

impl Drop for EventDrivenEditor {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
