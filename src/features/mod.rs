//! Features Module
//!
//! This module contains advanced editor features and functionality:
//! - Macro recording and playback system
//! - Syntax highlighting and language support
//! - Text objects and advanced text manipulation
//! - Search and completion capabilities
//! - Language Server Protocol integration

pub mod completion;
pub mod lsp;
pub mod macros;
pub mod search;
pub mod syntax;
pub mod syntax_manager;
pub mod text_objects;

pub use completion::CommandCompletion;
pub use lsp::*;
pub use macros::{Macro, MacroError, MacroKeyEvent, MacroRecorder};
pub use search::{SearchEngine, SearchResult};
pub use syntax::{AsyncSyntaxHighlighter, HighlightRange, Priority};
pub use syntax_manager::*;
pub use text_objects::*;
