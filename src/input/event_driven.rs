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

#[derive(Debug, Clone, PartialEq)]
struct RenderState {
    mode: crate::core::mode::Mode,
    cursor_position: Option<(usize, usize)>,
    buffer_id: Option<usize>,
    command_line: String,
    status_message: String,
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

        // Start syntax highlighting thread
        let syntax_thread = Self::spawn_syntax_thread(editor.clone(), event_sender.clone());
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
            match self.event_receiver.recv_timeout(Duration::from_millis(16)) {
                Ok(event) => {
                    let should_quit = self.process_event(event)?;
                    if should_quit {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Check if editor wants to quit
                    if let Ok(editor) = self.editor.try_lock()
                        && editor.should_quit()
                    {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    warn!("Event channel disconnected");
                    break;
                }
            }
        }

        self.shutdown()?;
        Ok(())
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
        match event {
            EditorEvent::Input(input_event) => self.handle_input_event(input_event),
            EditorEvent::UI(ui_event) => self.handle_ui_event(ui_event),
            EditorEvent::Buffer(buffer_event) => self.handle_buffer_event(buffer_event),
            EditorEvent::Window(window_event) => self.handle_window_event(window_event),
            EditorEvent::Config(config_event) => self.handle_config_event(config_event),
            EditorEvent::Search(search_event) => self.handle_search_event(search_event),
            EditorEvent::System(system_event) => self.handle_system_event(system_event),
            EditorEvent::Plugin(_) => Ok(false), // Future implementation
            EditorEvent::LSP(_) => Ok(false),    // Future implementation
            EditorEvent::Macro(macro_event) => self.handle_macro_event(macro_event),
        }
    }

    /// Handle input events
    fn handle_input_event(&self, event: InputEvent) -> Result<bool> {
        match event {
            InputEvent::KeyPress(key_event) => {
                if let Ok(mut editor) = self.editor.lock() {
                    // Store the editor state before processing input
                    let mode_before = editor.mode();
                    let cursor_before = editor.current_buffer().map(|b| b.cursor);
                    let buffer_id_before = editor.current_buffer_id;
                    let command_line_before = editor.command_line().to_string();
                    // Snapshot completion state before
                    let completion_active_before = editor.is_completion_active();
                    let completion_matches_len_before = editor.completion_matches_len();
                    let completion_selected_index_before = editor.completion_selected_index();

                    // Process the key event
                    editor.handle_key_event(key_event)?;

                    // Check what actually changed to decide if we need a redraw
                    let mode_after = editor.mode();
                    let cursor_after = editor.current_buffer().map(|b| b.cursor);
                    let buffer_id_after = editor.current_buffer_id;
                    let command_line_after = editor.command_line();
                    // Completion state after
                    let completion_active_after = editor.is_completion_active();
                    let completion_matches_len_after = editor.completion_matches_len();
                    let completion_selected_index_after = editor.completion_selected_index();

                    let needs_redraw = mode_before != mode_after
                        || cursor_before != cursor_after
                        || buffer_id_before != buffer_id_after
                        || command_line_before != command_line_after
                        // Redraw when completion state changes (activation, matches, selection)
                        || completion_active_before != completion_active_after
                        || completion_matches_len_before != completion_matches_len_after
                        || completion_selected_index_before != completion_selected_index_after
                        || editor
                            .needs_syntax_refresh
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
                    // Check what actually needs to be redrawn
                    let current_mode = editor.mode();
                    let current_cursor = editor
                        .current_buffer()
                        .map(|b| (b.cursor.row, b.cursor.col));
                    let current_buffer_id = editor.current_buffer_id;
                    let current_command_line = editor.command_line().to_string();
                    let current_status = editor.status_message().to_string();
                    // Current completion state summary
                    let current_completion_active = editor.is_completion_active();
                    let current_completion_matches_len = editor.completion_matches_len();
                    let current_completion_selected_index = editor.completion_selected_index();

                    let needs_redraw = last_state.mode != current_mode
                        || last_state.cursor_position != current_cursor
                        || last_state.buffer_id != current_buffer_id
                        || last_state.command_line != current_command_line
                        || last_state.status_message != current_status
                        // Redraw when completion state changed
                        || last_state.completion_active != current_completion_active
                        || last_state.completion_matches_len != current_completion_matches_len
                        || last_state.completion_selected_index
                            != current_completion_selected_index
                        || last_state.needs_full_redraw
                        || editor
                            .needs_syntax_refresh
                            .load(std::sync::atomic::Ordering::Relaxed);

                    if needs_redraw {
                        if let Err(e) = editor.render() {
                            error!("Render failed: {}", e);
                        } else {
                            // Update our cached state
                            last_state.mode = current_mode;
                            last_state.cursor_position = current_cursor;
                            last_state.buffer_id = current_buffer_id;
                            last_state.command_line = current_command_line;
                            last_state.status_message = current_status;
                            last_state.needs_full_redraw = false;
                            last_state.completion_active = current_completion_active;
                            last_state.completion_matches_len = current_completion_matches_len;
                            last_state.completion_selected_index =
                                current_completion_selected_index;

                            // Reset syntax refresh flag
                            editor
                                .needs_syntax_refresh
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
    fn handle_buffer_event(&self, _event: BufferEvent) -> Result<bool> {
        // TODO: Implement buffer event handling
        Ok(false)
    }

    /// Handle window events
    fn handle_window_event(&self, _event: WindowEvent) -> Result<bool> {
        // TODO: Implement window event handling
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
    fn handle_search_event(&self, _event: SearchEvent) -> Result<bool> {
        // TODO: Implement search event handling
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
                match event::poll(Duration::from_millis(16)) {
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

            loop {
                thread::sleep(Duration::from_millis(500)); // Check every 500ms

                if let Ok(editor) = editor.try_lock() {
                    if editor.should_quit() {
                        break;
                    }

                    // Check for config changes
                    if let Some(ref watcher) = editor.config_watcher {
                        let changes = watcher.check_for_changes();
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
                }

                thread::sleep(Duration::from_millis(100));
            }

            info!("Config watcher thread finished");
        })
    }

    /// Spawn syntax highlighting thread
    fn spawn_syntax_thread(
        editor: Arc<Mutex<Editor>>,
        sender: mpsc::Sender<EditorEvent>,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            info!("Syntax highlighting thread started");

            loop {
                thread::sleep(Duration::from_millis(100)); // Check every 100ms

                if let Ok(editor) = editor.try_lock() {
                    if editor.should_quit() {
                        break;
                    }

                    // Check if syntax refresh is needed
                    if editor
                        .needs_syntax_refresh
                        .load(std::sync::atomic::Ordering::Relaxed)
                    {
                        drop(editor); // Release lock before sending event

                        if let Err(e) = sender.send(EditorEvent::UI(UIEvent::RedrawRequest)) {
                            error!("Failed to send syntax refresh event: {}", e);
                            break;
                        }
                    }
                }

                thread::sleep(Duration::from_millis(50));
            }

            info!("Syntax highlighting thread finished");
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
            // Remove temporary loop structure
            thread::sleep(Duration::from_secs(1));

            info!("Render thread finished");
        })
    }

    /// Shutdown all background threads
    fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down event-driven editor");

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
                        log::info!("Started macro recording for register '{}'", register);
                    }
                }
                crate::input::events::MacroEvent::StopRecording => {
                    if editor.is_macro_recording() {
                        if let Err(e) = editor.stop_macro_recording() {
                            log::warn!("Failed to stop macro recording: {}", e);
                        } else {
                            log::info!("Stopped macro recording");
                        }
                    }
                }
                crate::input::events::MacroEvent::Execute { register, count: _ } => {
                    if let Err(e) = editor.play_macro(register) {
                        log::warn!("Failed to execute macro '{}': {}", register, e);
                    } else {
                        log::info!("Executed macro '{}'", register);
                    }
                }
                crate::input::events::MacroEvent::RepeatLast { count: _ } => {
                    if let Err(e) = editor.play_last_macro() {
                        log::warn!("Failed to repeat last macro: {}", e);
                    } else {
                        log::info!("Repeated last macro");
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
