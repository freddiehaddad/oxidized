use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigChangeEvent {
    EditorConfigChanged,
    KeymapConfigChanged,
    ThemeConfigChanged,
}

impl ConfigChangeEvent {
    /// Get the filename associated with this config change event
    pub fn filename(&self) -> &'static str {
        match self {
            ConfigChangeEvent::EditorConfigChanged => "editor.toml",
            ConfigChangeEvent::KeymapConfigChanged => "keymaps.toml",
            ConfigChangeEvent::ThemeConfigChanged => "themes.toml",
        }
    }
}

pub struct ConfigWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<ConfigChangeEvent>,
}

impl ConfigWatcher {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::channel();

        let watcher = Self::create_watcher(tx)?;

        Ok(ConfigWatcher {
            _watcher: watcher,
            receiver: rx,
        })
    }

    fn create_watcher(
        tx: Sender<ConfigChangeEvent>,
    ) -> Result<RecommendedWatcher, Box<dyn std::error::Error>> {
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| match res {
                Ok(event) => {
                    log::trace!("Config watcher received event: {:?}", event);
                    if let EventKind::Modify(_) = event.kind {
                        for path in event.paths {
                            if let Some(file_name) = path.file_name() {
                                match file_name.to_string_lossy().as_ref() {
                                    "editor.toml" => {
                                        log::debug!("Detected editor.toml modification");
                                        let _ = tx.send(ConfigChangeEvent::EditorConfigChanged);
                                    }
                                    "keymaps.toml" => {
                                        log::debug!("Detected keymaps.toml modification");
                                        let _ = tx.send(ConfigChangeEvent::KeymapConfigChanged);
                                    }
                                    "themes.toml" => {
                                        log::debug!("Detected themes.toml modification");
                                        let _ = tx.send(ConfigChangeEvent::ThemeConfigChanged);
                                    }
                                    _ => {
                                        log::trace!(
                                            "Ignoring modification of file: {:?}",
                                            file_name
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Config watcher error: {:?}", e);
                }
            },
            Config::default(),
        )?;

        // Watch specific config files instead of the entire directory to avoid log file feedback loops
        let editor_config_path = Path::new("editor.toml");
        let keymap_config_path = Path::new("keymaps.toml");
        let themes_config_path = Path::new("themes.toml");

        let mut watched_files = 0;

        // Only watch files that exist to avoid errors
        if editor_config_path.exists() {
            watcher.watch(editor_config_path, RecursiveMode::NonRecursive)?;
            log::info!("Watching editor config at: {:?}", editor_config_path);
            watched_files += 1;
        }
        if keymap_config_path.exists() {
            watcher.watch(keymap_config_path, RecursiveMode::NonRecursive)?;
            log::info!("Watching keymap config at: {:?}", keymap_config_path);
            watched_files += 1;
        }
        if themes_config_path.exists() {
            watcher.watch(themes_config_path, RecursiveMode::NonRecursive)?;
            log::info!("Watching themes config at: {:?}", themes_config_path);
            watched_files += 1;
        }

        log::info!(
            "Config watcher initialized, watching {} config files",
            watched_files
        );

        Ok(watcher)
    }

    /// Check for configuration changes (non-blocking)
    pub fn check_for_changes(&self) -> Vec<ConfigChangeEvent> {
        let mut changes = Vec::new();

        // Collect all pending events
        while let Ok(event) = self.receiver.try_recv() {
            changes.push(event);
        }

        if !changes.is_empty() {
            log::debug!("Found {} config changes", changes.len());
        }

        changes
    }

    /// Check if themes.toml has changed (non-blocking)
    pub fn check_for_theme_changes(&self) -> bool {
        let changes = self.check_for_changes();
        changes.contains(&ConfigChangeEvent::ThemeConfigChanged)
    }

    /// Check if editor.toml has changed (non-blocking)
    pub fn check_for_editor_changes(&self) -> bool {
        let changes = self.check_for_changes();
        changes.contains(&ConfigChangeEvent::EditorConfigChanged)
    }

    /// Check if keymaps.toml has changed (non-blocking)
    pub fn check_for_keymap_changes(&self) -> bool {
        let changes = self.check_for_changes();
        changes.contains(&ConfigChangeEvent::KeymapConfigChanged)
    }

    /// Wait for a configuration change with timeout
    pub fn wait_for_change(&self, timeout: Duration) -> Option<ConfigChangeEvent> {
        match self.receiver.recv_timeout(timeout) {
            Ok(event) => {
                log::debug!("Config change detected: {:?}", event);
                Some(event)
            }
            Err(_) => None,
        }
    }

    /// Wait indefinitely for the next configuration change.
    /// Returns None if the watcher channel is disconnected (e.g., on shutdown).
    pub fn wait_for_change_blocking(&self) -> Option<ConfigChangeEvent> {
        match self.receiver.recv() {
            Ok(event) => {
                log::debug!("Config change detected: {:?}", event);
                Some(event)
            }
            Err(_) => None,
        }
    }
}
