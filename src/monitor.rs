use crate::config::{Config, expand_path};
use crate::error::Result;
use notify::{Event, EventKind, RecursiveMode, Watcher, recommended_watcher};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use tokio::sync::mpsc;
use tracing::{info, warn, error};

#[derive(Debug, Clone)]
pub struct FileEvent {
    pub path: PathBuf,
    pub event_type: FileEventType,
}

#[derive(Debug, Clone)]
pub enum FileEventType {
    Created,
    Modified,
    Deleted,
}

pub struct DirectoryMonitor {
    config: Config,
    event_sender: mpsc::UnboundedSender<FileEvent>,
}

impl DirectoryMonitor {
    pub fn new(config: Config, event_sender: mpsc::UnboundedSender<FileEvent>) -> Result<Self> {
        Ok(Self { config, event_sender })
    }

    pub async fn start(&self) -> Result<()> {
        let (tx, rx) = channel();
        let mut watcher = recommended_watcher(tx)?;

        for dir in &self.config.directories {
            let expanded_path = expand_path(&dir.path);
            
            if !expanded_path.exists() {
                warn!("Directory does not exist: {:?}", expanded_path);
                continue;
            }

            let mode = if dir.recursive {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };

            watcher.watch(&expanded_path, mode)?;
            info!("Watching directory: {:?} (recursive: {})", expanded_path, dir.recursive);
        }

        let event_sender = self.event_sender.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            while let Ok(event) = rx.recv() {
                match event {
                    Ok(event) => {
                        if let Some(file_event) = DirectoryMonitor::process_event_static(event, &config) {
                            if let Err(e) = event_sender.send(file_event) {
                                error!("Failed to send file event: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Watch error: {:?}", e);
                    }
                }
            }
        });

        Ok(())
    }

    pub fn process_event_static(event: Event, config: &Config) -> Option<FileEvent> {
        let path = event.paths.first()?;

        // Only process files, not directories
        if path.is_dir() {
            return None;
        }

        // Check if file matches any monitored directory's patterns
        let monitored_dir = config.directories.iter().find(|dir| {
            let expanded_path = expand_path(&dir.path);
            path.starts_with(&expanded_path)
        })?;

        let file_name = path.file_name()?.to_str()?;
        let matches_pattern = monitored_dir.file_patterns.is_empty()
            || monitored_dir.file_patterns.iter().any(|pattern| {
                DirectoryMonitor::matches_pattern(file_name, pattern)
            });

        if !matches_pattern {
            return None;
        }

        let event_type = match event.kind {
            EventKind::Create(_) => FileEventType::Created,
            EventKind::Modify(_) => FileEventType::Modified,
            EventKind::Remove(_) => FileEventType::Deleted,
            _ => return None,
        };

        Some(FileEvent {
            path: path.clone(),
            event_type,
        })
    }

    pub fn matches_pattern(file_name: &str, pattern: &str) -> bool {
        // Simple glob-like matching
        if pattern == "*" {
            return true;
        }

        if pattern.starts_with("*.") {
            let ext = &pattern[2..];
            return file_name.ends_with(ext);
        }

        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                return file_name.starts_with(parts[0]) && file_name.ends_with(parts[1]);
            }
        }

        file_name == pattern
    }
}
