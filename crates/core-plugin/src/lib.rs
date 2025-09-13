//! Plugin host abstraction scaffold (Refactor R4 Step 16).
//!
//! Breadth-first stub introducing a minimal `PluginHost` trait so future dynamic
//! runtime extensions (LSP, formatting tools, AI, diagnostics, git integration)
//! have a stable conceptual anchor. This step deliberately avoids any loading
//! mechanics (filesystem discovery, WASI sandbox, dynamic linking) or RPC
//! protocol plumbing. It only establishes the trait surface and a `NoopPluginHost`
//! implementation used by the main runtime until real functionality lands.
//!
//! Design Notes:
//! - Kept intentionally tiny: name + load_all + event_sources.
//! - `load_all` returns Result<()> to reserve space for IO / parse failures.
//! - `event_sources` returns owned boxed `AsyncEventSource` objects allowing the
//!   caller (registry) to spawn them uniformly alongside built-ins.
//! - No async fn yet: loading is synchronous for now; we can introduce an async
//!   variant (or internal tokio tasks) once real plugin discovery is implemented.
//! - This crate depends only on `core-events` to avoid cycles.
//!
//! Extension Path (Deferred): actual discovery (filesystem scan, config‑declared
//! manifests, dynamic linking, WASI modules) and sandboxing will populate an
//! internal collection of plugin descriptors. Each descriptor may expose zero or
//! more async event sources bridged into the global `EventSourceRegistry` via
//! `event_sources()`. Command, status segment, and style span contributions will
//! gain analogous composition seams once those feature phases begin.

use core_events::AsyncEventSource;

/// Trait representing a collection-oriented plugin host. Implementors are
/// responsible for discovering zero or more plugins (from disk, config, or
/// compiled-in) and exposing any asynchronous event sources they contribute.
pub trait PluginHost: Send + Sync {
    /// Stable human-readable host identifier (for logs / diagnostics).
    fn name(&self) -> &'static str;
    /// Load / discover plugins. Breadth-first: default hosts do nothing. Implementors
    /// SHOULD be idempotent; repeated calls may either short-circuit or revalidate
    /// state but must not duplicate event sources.
    fn load_all(&mut self) -> anyhow::Result<()>;
    /// Extract any async event sources contributed by loaded plugins. Ownership is
    /// transferred to the caller. Subsequent calls after extraction SHOULD return
    /// an empty Vec.
    fn event_sources(&mut self) -> Vec<Box<dyn AsyncEventSource>>;
}

impl<T: PluginHost + ?Sized> PluginHost for &mut T {
    fn name(&self) -> &'static str {
        (**self).name()
    }
    fn load_all(&mut self) -> anyhow::Result<()> {
        (**self).load_all()
    }
    fn event_sources(&mut self) -> Vec<Box<dyn AsyncEventSource>> {
        (**self).event_sources()
    }
}

/// No‑op host used until real plugin discovery lands.
#[derive(Default)]
pub struct NoopPluginHost {
    drained: bool,
}

impl NoopPluginHost {
    pub fn new() -> Self {
        Self { drained: false }
    }
}

impl PluginHost for NoopPluginHost {
    fn name(&self) -> &'static str {
        "noop-plugin-host"
    }
    fn load_all(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    fn event_sources(&mut self) -> Vec<Box<dyn AsyncEventSource>> {
        if self.drained {
            return Vec::new();
        }
        self.drained = true; // future: drain collected sources
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_host_load_and_sources_empty() {
        let mut host = NoopPluginHost::new();
        host.load_all().expect("noop load should succeed");
        let first = host.event_sources();
        assert!(first.is_empty(), "noop produces no sources");
        let second = host.event_sources();
        assert!(second.is_empty(), "subsequent calls remain empty");
    }
}
