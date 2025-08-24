// Event-driven architecture for the oxidized editor
// This module provides the foundation for converting from the current imperative model
// to an event-driven architecture like Neovim and Vim

use crate::core::mode::Mode;
use crate::features::search::SearchResult;
use crossterm::event::KeyEvent;
use std::path::PathBuf;

/// Core event types that can occur in the editor
#[derive(Debug, Clone)]
pub enum EditorEvent {
    /// Input events
    Input(InputEvent),
    /// Buffer events
    Buffer(BufferEvent),
    /// UI events  
    UI(UIEvent),
    /// Window events
    Window(WindowEvent),
    /// Configuration events
    Config(ConfigEvent),
    /// Search events
    Search(SearchEvent),
    /// Macro events
    Macro(MacroEvent),
    /// System events
    System(SystemEvent),
    /// Plugin events (future extensibility)
    Plugin(PluginEvent),
    /// LSP events (future LSP integration)
    LSP(LSPEvent),
    /// Syntax results ready (wake UI)
    SyntaxReady,
}

/// Macro-related events
#[derive(Debug, Clone)]
pub enum MacroEvent {
    /// Start recording a macro to the specified register
    StartRecording(char),
    /// Stop recording the current macro
    StopRecording,
    /// Execute a macro from the specified register
    Execute { register: char, count: usize },
    /// Repeat the last executed macro
    RepeatLast { count: usize },
}

/// Input-related events
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Raw key press from terminal
    KeyPress(KeyEvent),
    /// Parsed key sequence (e.g., "gg", "dd", "2yy")
    KeySequence(String),
    /// Mode transition request
    ModeChange { from: Mode, to: Mode },
    /// Command entered in command mode
    Command(String),
    /// Text insertion in insert mode
    TextInsert {
        text: String,
        position: (usize, usize),
    },
}

/// Buffer-related events
#[derive(Debug, Clone)]
pub enum BufferEvent {
    /// Buffer created
    Created {
        buffer_id: usize,
        path: Option<PathBuf>,
    },
    /// Buffer opened from file
    Opened { buffer_id: usize, path: PathBuf },
    /// Buffer modified
    Modified { buffer_id: usize },
    /// Buffer saved
    Saved { buffer_id: usize, path: PathBuf },
    /// Buffer closed
    Closed { buffer_id: usize },
    /// Content changed
    ContentChanged {
        buffer_id: usize,
        line: usize,
        col: usize,
        old_text: String,
        new_text: String,
    },
    /// Cursor moved
    CursorMoved {
        buffer_id: usize,
        old_pos: (usize, usize),
        new_pos: (usize, usize),
    },
    /// Selection changed
    SelectionChanged {
        buffer_id: usize,
        start: Option<(usize, usize)>,
        end: Option<(usize, usize)>,
    },
    /// Syntax highlighting updated
    SyntaxHighlighted {
        buffer_id: usize,
        line: usize,
        highlights: Vec<crate::features::syntax::HighlightRange>,
    },
}

/// UI-related events
#[derive(Debug, Clone)]
pub enum UIEvent {
    /// Terminal resized
    Resize { width: u16, height: u16 },
    /// Redraw requested
    RedrawRequest,
    /// Theme changed
    ThemeChanged(String),
    /// Status message updated
    StatusMessage(String),
    /// Command line updated
    CommandLineUpdated(String),
    /// Viewport changed (scrolling)
    ViewportChanged {
        buffer_id: usize,
        top: usize,
        visible_lines: usize,
    },
}

/// Window management events
#[derive(Debug, Clone)]
pub enum WindowEvent {
    /// Window created
    Created { window_id: usize },
    /// Window closed
    Closed { window_id: usize },
    /// Window split
    Split {
        parent_id: usize,
        new_window_id: usize,
        direction: crate::core::window::SplitDirection,
    },
    /// Window focus changed
    FocusChanged {
        old_window_id: Option<usize>,
        new_window_id: usize,
    },
    /// Window resized
    Resized {
        window_id: usize,
        width: u16,
        height: u16,
    },
}

/// Configuration events
#[derive(Debug, Clone)]
pub enum ConfigEvent {
    /// Editor config file changed
    EditorConfigChanged,
    /// Theme config file changed
    ThemeConfigChanged,
    /// Keymap config file changed
    KeymapConfigChanged,
    /// Setting changed via :set command
    SettingChanged { key: String, value: String },
}

/// Search-related events
#[derive(Debug, Clone)]
pub enum SearchEvent {
    /// Search started
    Started { pattern: String, is_regex: bool },
    /// Search results found
    ResultsFound(Vec<SearchResult>),
    /// Search navigation (n/N)
    Navigate { direction: SearchDirection },
    /// Search cancelled
    Cancelled,
}

#[derive(Debug, Clone)]
pub enum SearchDirection {
    Forward,
    Backward,
}

/// System-level events
#[derive(Debug, Clone)]
pub enum SystemEvent {
    /// Request to quit the editor
    Quit,
    /// Force quit without saving
    ForceQuit,
    /// File system event (file changed externally)
    FileChanged(PathBuf),
    /// Timer event (for periodic tasks)
    Timer { id: String },
    /// Signal received (SIGTERM, etc.)
    Signal(String),
}

/// Plugin system events (future extensibility)
#[derive(Debug, Clone)]
pub enum PluginEvent {
    /// Plugin loaded
    Loaded(String),
    /// Plugin command
    Command { plugin: String, command: String },
    /// Custom event from plugin
    Custom {
        plugin: String,
        data: serde_json::Value,
    },
}

/// LSP events (future Language Server Protocol integration)
#[derive(Debug, Clone)]
pub enum LSPEvent {
    /// LSP server started
    ServerStarted { language: String },
    /// Diagnostics received
    Diagnostics {
        buffer_id: usize,
        diagnostics: Vec<LSPDiagnostic>,
    },
    /// Completion received
    Completion {
        buffer_id: usize,
        position: (usize, usize),
        items: Vec<LSPCompletionItem>,
    },
    /// Hover information
    Hover {
        buffer_id: usize,
        position: (usize, usize),
        content: String,
    },
}

#[derive(Debug, Clone)]
pub struct LSPDiagnostic {
    pub line: usize,
    pub col: usize,
    pub length: usize,
    pub severity: LSPDiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum LSPDiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Debug, Clone)]
pub struct LSPCompletionItem {
    pub label: String,
    pub kind: LSPCompletionKind,
    pub detail: Option<String>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone)]
pub enum LSPCompletionKind {
    Text,
    Method,
    Function,
    Constructor,
    Field,
    Variable,
    Class,
    Interface,
    Module,
    Property,
    Unit,
    Value,
    Enum,
    Keyword,
    Snippet,
    Color,
    File,
    Reference,
}

/// Convenience macros for creating events
#[macro_export]
macro_rules! event {
    (input::key_press($key:expr)) => {
        EditorEvent::Input(InputEvent::KeyPress($key))
    };
    (buffer::modified($id:expr)) => {
        EditorEvent::Buffer(BufferEvent::Modified { buffer_id: $id })
    };
    (ui::redraw) => {
        EditorEvent::UI(UIEvent::RedrawRequest)
    };
    (system::quit) => {
        EditorEvent::System(SystemEvent::Quit)
    };
}
