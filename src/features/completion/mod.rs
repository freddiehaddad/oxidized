//! Modular command completion system
//!
//! Decomposed into:
//! - schema: canonical option schema and alias mapping
//! - providers: pluggable static and dynamic item providers via a registry
//! - presenter: normalization/deduplication/sorting independent of providers
//! - engine: state machine facade providing the public API over providers+presenter

pub mod engine;
pub mod presenter;
pub mod providers;
pub mod schema;

pub use engine::{
    BufferSummary, CommandCompletion, CommandCompletionBuilder, CompletionContext, CompletionItem,
};
pub use presenter::{CompletionPresenter, DefaultPresenter};
pub use providers::{CompletionProvider, ProviderRegistry};
