//! Configuration loading (Phase 0 stub)

use anyhow::Result;
use std::{fs, path::PathBuf};

#[derive(Debug, Default)]
pub struct Config {
    pub raw: Option<String>,
}

/// Best-effort config path following platform conventions (XDG / AppData Roaming).
pub fn discover() -> PathBuf {
    if let Some(dir) = dirs::config_dir() {
        return dir.join("oxidized").join("config.toml");
    }
    // Fallback to a relative file if a standard config directory cannot be determined.
    PathBuf::from("oxidized-config.toml")
}

pub fn load() -> Result<Config> {
    let path = discover();
    if let Ok(content) = fs::read_to_string(path) {
        Ok(Config { raw: Some(content) })
    } else {
        Ok(Config { raw: None })
    }
}
