//! Configuration loading and parsing (Phase 2 Step 14)
//!
//! Scope (Step 14): Parse `oxidized.toml` (or override path provided by the
//! binary) extracting `[scroll.margin] vertical = <u16>` with default 0 when
//! absent. We clamp the effective value at `(h - 2) / 2` where `h` is the
//! current viewport height provided by the caller at application time. The
//! clamp logic lives in `Config::apply_context`. The raw parsed value
//! (pre‑clamp) is retained so future dynamic viewport changes can re-clamp.
//!
//! Breadth-first: only vertical margin implemented; horizontal and other
//! scroll behaviors deferred. Unknown fields are ignored (TOML deserialization
//! tolerance) to allow forward evolution without immediate warnings.

use anyhow::Result;
use serde::Deserialize;
use std::{fs, path::PathBuf};
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConfigPlatformTraits {
    pub is_windows: bool,
    pub supports_scroll_region: bool,
}

impl ConfigPlatformTraits {
    pub const fn new(is_windows: bool, supports_scroll_region: bool) -> Self {
        Self {
            is_windows,
            supports_scroll_region,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigContext {
    pub viewport_columns: u16,
    pub viewport_rows: u16,
    pub status_rows: u16,
    pub overlay_rows: u16,
    pub platform: ConfigPlatformTraits,
}

impl ConfigContext {
    pub fn new(
        viewport_columns: u16,
        viewport_rows: u16,
        status_rows: u16,
        overlay_rows: u16,
        platform: ConfigPlatformTraits,
    ) -> Self {
        Self {
            viewport_columns,
            viewport_rows,
            status_rows,
            overlay_rows,
            platform,
        }
    }

    pub fn text_rows(&self) -> u16 {
        let reserved = self.status_rows.saturating_add(self.overlay_rows);
        self.viewport_rows.saturating_sub(reserved)
    }

    pub fn from_viewport_height(viewport_rows: u16) -> Self {
        Self {
            viewport_columns: 0,
            viewport_rows,
            status_rows: 0,
            overlay_rows: 0,
            platform: ConfigPlatformTraits::default(),
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct MarginConfig {
    #[serde(default)]
    pub vertical: u16,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct ScrollConfig {
    #[serde(default)]
    pub margin: MarginConfig,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct ConfigFile {
    #[serde(default)]
    pub scroll: ScrollConfig,
    #[serde(default)]
    pub input: InputConfig,
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub raw: Option<String>,            // original file string (optional)
    pub file: ConfigFile,               // parsed (or default) data
    pub effective_vertical_margin: u16, // clamped to viewport semantics
}

#[derive(Debug, Deserialize, Clone)]
pub struct InputConfig {
    #[serde(default = "InputConfig::default_timeout")] // Vim default: enabled
    pub timeout: bool,
    #[serde(default = "InputConfig::default_timeoutlen")] // Vim default usually 1000ms
    pub timeoutlen: u32,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            timeout: Self::default_timeout(),
            timeoutlen: Self::default_timeoutlen(),
        }
    }
}

impl InputConfig {
    const fn default_timeout() -> bool {
        true
    }
    const fn default_timeoutlen() -> u32 {
        1000
    }
}
/// Best-effort config path following platform conventions (XDG / AppData Roaming).
pub fn discover() -> PathBuf {
    // Phase 2 spec: prefer local working directory `oxidized.toml` before
    // falling back to platform config dir for this minimal step.
    let local = PathBuf::from("oxidized.toml");
    if local.exists() {
        return local;
    }
    if let Some(dir) = dirs::config_dir() {
        return dir.join("oxidized").join("oxidized.toml");
    }
    // Final fallback relative filename.
    PathBuf::from("oxidized.toml")
}

pub fn load_from(path: Option<PathBuf>) -> Result<Config> {
    let path = path.unwrap_or_else(discover);
    if let Ok(content) = fs::read_to_string(&path) {
        match toml::from_str::<ConfigFile>(&content) {
            Ok(file) => Ok(Config {
                raw: Some(content),
                file,
                effective_vertical_margin: 0, // computed later
            }),
            Err(_e) => {
                // On parse error fallback to defaults (breadth-first resilience).
                Ok(Config::default())
            }
        }
    } else {
        Ok(Config::default())
    }
}

impl Config {
    /// Apply viewport + platform context to compute clamped vertical margin.
    /// Returns the effective (possibly clamped) value.
    pub fn apply_context(&mut self, ctx: ConfigContext) -> u16 {
        let raw = self.file.scroll.margin.vertical;
        let text_rows = ctx.text_rows();
        let (clamped, max) = if text_rows <= 3 {
            (0, 0)
        } else {
            let max = (text_rows.saturating_sub(2)) / 2; // (h - 2)/2 using text rows
            (raw.min(max), max)
        };

        if clamped != raw {
            info!(
                target: "config",
                raw,
                clamped,
                max,
                text_rows,
                viewport_rows = ctx.viewport_rows,
                overlay_rows = ctx.overlay_rows,
                status_rows = ctx.status_rows,
                supports_scroll_region = ctx.platform.supports_scroll_region,
                is_windows = ctx.platform.is_windows,
                "scroll_margin_vertical_clamped"
            );
        }
        self.effective_vertical_margin = clamped;
        clamped
    }

    /// Back-compat wrapper for pre-context API: apply viewport height only.
    pub fn apply_viewport_height(&mut self, viewport_height: u16) -> u16 {
        self.apply_context(ConfigContext::from_viewport_height(viewport_height))
    }

    /// Recompute effective vertical margin on a viewport or platform change. Returns
    /// `Some(new_margin)` when the effective value changed, else `None`.
    pub fn recompute_with_context(&mut self, ctx: ConfigContext) -> Option<u16> {
        let prev = self.effective_vertical_margin;
        let current = self.apply_context(ctx);
        if current != prev { Some(current) } else { None }
    }

    /// Back-compat wrapper using viewport height only.
    pub fn recompute_after_resize(&mut self, new_viewport_height: u16) -> Option<u16> {
        self.recompute_with_context(ConfigContext::from_viewport_height(new_viewport_height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, MutexGuard};
    use tracing::Level;
    use tracing::subscriber::with_default;
    use tracing_subscriber::fmt::MakeWriter;

    fn ctx_with_text_rows(rows: u16) -> ConfigContext {
        ConfigContext::new(80, rows, 0, 0, ConfigPlatformTraits::default())
    }

    #[derive(Clone)]
    struct BufferWriter {
        inner: Arc<Mutex<Vec<u8>>>,
    }

    impl BufferWriter {
        fn new() -> (Self, Arc<Mutex<Vec<u8>>>) {
            let buf = Arc::new(Mutex::new(Vec::new()));
            (Self { inner: buf.clone() }, buf)
        }
    }

    struct LockedWriter<'a> {
        guard: MutexGuard<'a, Vec<u8>>,
    }

    impl<'a> Write for LockedWriter<'a> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.guard.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for BufferWriter {
        type Writer = LockedWriter<'a>;

        fn make_writer(&'a self) -> Self::Writer {
            LockedWriter {
                guard: self.inner.lock().expect("log buffer poisoned"),
            }
        }
    }

    #[test]
    fn default_config_when_missing_file() {
        let cfg = load_from(Some(PathBuf::from("__nonexistent_hopefully__.toml"))).unwrap();
        assert_eq!(cfg.file.scroll.margin.vertical, 0);
    }

    #[test]
    fn parses_vertical_margin_value() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "[scroll.margin]\nvertical = 3\n").unwrap();
        let mut cfg = load_from(Some(tmp.path().to_path_buf())).unwrap();
        assert_eq!(cfg.file.scroll.margin.vertical, 3);
        cfg.apply_context(ctx_with_text_rows(40)); // ample height, no clamp
        assert_eq!(cfg.effective_vertical_margin, 3);
    }

    #[test]
    fn clamps_when_value_exceeds_half_minus_one() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "[scroll.margin]\nvertical = 50\n").unwrap();
        let mut cfg = load_from(Some(tmp.path().to_path_buf())).unwrap();
        // viewport height 20 -> max = (20 - 2)/2 = 9
        let eff = cfg.apply_context(ctx_with_text_rows(20));
        assert_eq!(eff, 9);
        assert_eq!(cfg.effective_vertical_margin, 9);
    }

    #[test]
    fn recompute_with_context_changes_when_height_shrinks() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "[scroll.margin]\nvertical = 10\n").unwrap();
        let mut cfg = load_from(Some(tmp.path().to_path_buf())).unwrap();
        cfg.apply_context(ctx_with_text_rows(50)); // plenty of room, margin=10
        assert_eq!(cfg.effective_vertical_margin, 10);
        // Shrink height so max decreases below 10: text rows reduce to 10 -> max=(10-2)/2=4
        let changed = cfg.recompute_with_context(ctx_with_text_rows(10));
        assert_eq!(changed, Some(4));
        assert_eq!(cfg.effective_vertical_margin, 4);
        // Another resize to slightly larger but same cap should keep value stable
        let changed2 = cfg.recompute_with_context(ctx_with_text_rows(11)); // max=(11-2)/2=4
        assert_eq!(changed2, None);
    }

    #[test]
    fn clamp_logging_uses_config_target() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "[scroll.margin]\nvertical = 8\n").unwrap();
        let mut cfg = load_from(Some(tmp.path().to_path_buf())).unwrap();
        let (writer, buffer) = BufferWriter::new();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(Level::INFO)
            .with_target(true)
            .with_ansi(false)
            .without_time()
            .with_writer(writer)
            .finish();

        with_default(subscriber, || {
            // Text rows small enough to force clamp: rows=6 -> max=(6-2)/2 = 2
            cfg.apply_context(ConfigContext::new(
                80,
                7,
                1,
                0,
                ConfigPlatformTraits::new(false, true),
            ));
        });

        let log_output = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        assert!(log_output.contains("INFO config:"));
        assert!(log_output.contains("scroll_margin_vertical_clamped"));
        assert_eq!(cfg.effective_vertical_margin, 2);
    }

    #[test]
    fn input_defaults_present() {
        let cfg = load_from(Some(PathBuf::from("__nonexistent_timeouts__.toml"))).unwrap();
        assert!(cfg.file.input.timeout);
        assert_eq!(cfg.file.input.timeoutlen, 1000);
    }

    #[test]
    fn parses_input_timeout_fields() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            tmp.path(),
            "[input]\ntimeout = false\ntimeoutlen = 250\n[scroll.margin]\nvertical = 3\n",
        )
        .unwrap();
        let cfg = load_from(Some(tmp.path().to_path_buf())).unwrap();
        assert!(!cfg.file.input.timeout);
        assert_eq!(cfg.file.input.timeoutlen, 250);
        assert_eq!(cfg.file.scroll.margin.vertical, 3);
    }
}
