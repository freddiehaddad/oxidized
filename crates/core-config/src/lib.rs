//! Configuration loading and parsing (Phase 2 Step 14)
//!
//! Scope (Step 14): Parse `oxidized.toml` (or override path provided by the
//! binary) extracting `[scroll.margin] vertical = <u16>` with default 0 when
//! absent. We clamp the effective value at `(h - 2) / 2` where `h` is the
//! current viewport height provided by the caller at application time. The
//! clamp logic lives in `Config::apply_viewport_height`. The raw parsed value
//! (pre‑clamp) is retained so future dynamic viewport changes can re-clamp.
//!
//! Breadth-first: only vertical margin implemented; horizontal and other
//! scroll behaviors deferred. Unknown fields are ignored (TOML deserialization
//! tolerance) to allow forward evolution without immediate warnings.

use anyhow::Result;
use serde::Deserialize;
use std::{fs, path::PathBuf};
use tracing::info;

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
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub raw: Option<String>,            // original file string (optional)
    pub file: ConfigFile,               // parsed (or default) data
    pub effective_vertical_margin: u16, // clamped to viewport semantics
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
    /// Apply viewport height to compute clamped vertical margin once per startup
    /// (Phase 2 Step 14). Returns the effective (possibly clamped) value.
    pub fn apply_viewport_height(&mut self, viewport_height: u16) -> u16 {
        let raw = self.file.scroll.margin.vertical;
        if viewport_height <= 3 {
            self.effective_vertical_margin = 0;
            return 0;
        }
        let max = (viewport_height.saturating_sub(2)) / 2; // (h - 2)/2
        let clamped = raw.min(max);
        if clamped != raw {
            info!(raw, clamped, max, "scroll_margin_vertical_clamped");
        }
        self.effective_vertical_margin = clamped;
        clamped
    }

    /// Recompute effective vertical margin on a viewport height change (Refactor R2 Step 7).
    /// Returns `Some(new_margin)` if the effective value changed, else `None`.
    pub fn recompute_after_resize(&mut self, new_viewport_height: u16) -> Option<u16> {
        let prev = self.effective_vertical_margin;
        let _ = self.apply_viewport_height(new_viewport_height); // reuse clamp logic
        if self.effective_vertical_margin != prev {
            Some(self.effective_vertical_margin)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        cfg.apply_viewport_height(40); // ample height, no clamp
        assert_eq!(cfg.effective_vertical_margin, 3);
    }

    #[test]
    fn clamps_when_value_exceeds_half_minus_one() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "[scroll.margin]\nvertical = 50\n").unwrap();
        let mut cfg = load_from(Some(tmp.path().to_path_buf())).unwrap();
        // viewport height 20 -> max = (20 - 2)/2 = 9
        let eff = cfg.apply_viewport_height(20);
        assert_eq!(eff, 9);
        assert_eq!(cfg.effective_vertical_margin, 9);
    }

    #[test]
    fn recompute_after_resize_changes_when_height_shrinks() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "[scroll.margin]\nvertical = 10\n").unwrap();
        let mut cfg = load_from(Some(tmp.path().to_path_buf())).unwrap();
        cfg.apply_viewport_height(50); // plenty of room, margin=10
        assert_eq!(cfg.effective_vertical_margin, 10);
        // Shrink height so max decreases below 10: height=10 -> max=(10-2)/2=4
        let changed = cfg.recompute_after_resize(10);
        assert_eq!(changed, Some(4));
        assert_eq!(cfg.effective_vertical_margin, 4);
        // Another resize to slightly larger but same cap should keep value stable
        let changed2 = cfg.recompute_after_resize(11); // max=(11-2)/2=4
        assert_eq!(changed2, None);
    }
}
