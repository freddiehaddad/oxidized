// File operations and management
// This will handle file I/O, directory browsing, etc.

use anyhow::Result;
use log::debug;
use std::fs;
use std::path::{Path, PathBuf};

pub struct FileManager {
    current_dir: PathBuf,
}

impl FileManager {
    pub fn new() -> Result<Self> {
        let current_dir = std::env::current_dir()?;
        debug!("Initialized FileManager with directory: {:?}", current_dir);
        Ok(Self { current_dir })
    }

    pub fn read_file<P: AsRef<Path>>(path: P) -> Result<String> {
        let path = path.as_ref();
        debug!("Reading file: {:?}", path);
        let content = fs::read_to_string(path)?;
        debug!(
            "Successfully read {} bytes from file: {:?}",
            content.len(),
            path
        );
        Ok(content)
    }

    pub fn write_file<P: AsRef<Path>>(path: P, content: &str) -> Result<()> {
        let path = path.as_ref();
        debug!("Writing {} bytes to file: {:?}", content.len(), path);
        fs::write(path, content)?;
        debug!("Successfully wrote file: {:?}", path);
        Ok(())
    }

    pub fn list_directory<P: AsRef<Path>>(&self, path: P) -> Result<Vec<DirEntry>> {
        let mut entries = Vec::new();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;

            entries.push(DirEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                path: entry.path(),
                is_dir: metadata.is_dir(),
                size: metadata.len(),
            });
        }

        // Sort directories first, then files
        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        Ok(entries)
    }

    pub fn current_directory(&self) -> &Path {
        &self.current_dir
    }

    pub fn change_directory<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let new_dir = path.as_ref().canonicalize()?;
        self.current_dir = new_dir;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}
