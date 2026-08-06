use crate::ai_client::{AiClient, ClarityImprovement, ConceptualizationResult};
use crate::config::{Config, expand_path};
use crate::error::Result;
use crate::monitor::{FileEvent, FileEventType};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{info, error};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingResult {
    pub id: String,
    pub file_path: PathBuf,
    pub timestamp: chrono::DateTime<Utc>,
    pub conceptualization: Option<ConceptualizationResult>,
    pub clarity_improvement: Option<ClarityImprovement>,
    pub processing_time_ms: u64,
    pub status: ProcessingStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessingStatus {
    Completed,
    Failed(String),
    Skipped(String),
}

pub struct DocumentProcessor {
    config: Config,
    ai_client: AiClient,
    event_receiver: UnboundedReceiver<FileEvent>,
    pending_files: std::collections::HashMap<PathBuf, Instant>,
}

impl DocumentProcessor {
    pub fn new(
        config: Config,
        ai_client: AiClient,
        event_receiver: UnboundedReceiver<FileEvent>,
    ) -> Self {
        Self {
            config,
            ai_client,
            event_receiver,
            pending_files: std::collections::HashMap::new(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let debounce_duration = Duration::from_millis(self.config.processing.debounce_interval_ms);

        loop {
            tokio::select! {
                Some(event) = self.event_receiver.recv() => {
                    self.handle_file_event(event, debounce_duration).await;
                }
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    self.process_pending_files().await?;
                }
            }
        }
    }

    async fn handle_file_event(&mut self, event: FileEvent, _debounce_duration: Duration) {
        match event.event_type {
            FileEventType::Created | FileEventType::Modified => {
                self.pending_files.insert(event.path.clone(), Instant::now());
                info!("File queued for processing: {:?}", event.path);
            }
            FileEventType::Deleted => {
                self.pending_files.remove(&event.path);
                info!("File removed from queue: {:?}", event.path);
            }
        }
    }

    async fn process_pending_files(&mut self) -> Result<()> {
        let debounce_duration = Duration::from_millis(self.config.processing.debounce_interval_ms);
        let now = Instant::now();

        let ready_to_process: Vec<PathBuf> = self
            .pending_files
            .iter()
            .filter(|(_, timestamp)| now.duration_since(**timestamp) >= debounce_duration)
            .map(|(path, _)| path.clone())
            .collect();

        for path in ready_to_process {
            self.pending_files.remove(&path);
            
            match self.process_file(&path).await {
                Ok(result) => {
                    self.save_result(&result).await?;
                    info!("Processed file: {:?} (took {}ms)", path, result.processing_time_ms);
                }
                Err(e) => {
                    error!("Failed to process file {:?}: {}", path, e);
                }
            }
        }

        Ok(())
    }

    async fn process_file(&self, path: &Path) -> Result<ProcessingResult> {
        let start = Instant::now();
        let id = Uuid::new_v4().to_string();

        // Check file size
        let metadata = fs::metadata(path)?;
        let size_mb = metadata.len() / (1024 * 1024);
        if size_mb > self.config.processing.max_file_size_mb {
            return Ok(ProcessingResult {
                id,
                file_path: path.to_path_buf(),
                timestamp: Utc::now(),
                conceptualization: None,
                clarity_improvement: None,
                processing_time_ms: start.elapsed().as_millis() as u64,
                status: ProcessingStatus::Skipped(format!(
                    "File too large ({}MB > {}MB limit)",
                    size_mb, self.config.processing.max_file_size_mb
                )),
            });
        }

        // Read file content
        let content = fs::read_to_string(path)?;

        // Determine file type and process accordingly
        let file_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let (conceptualization, clarity_improvement) = match file_ext {
            "txt" | "md" | "rst" | "asciidoc" => {
                let concept = self.ai_client.conceptualize_document(&content).await.ok();
                let clarity = self.ai_client.improve_clarity_with_llama(&content).await.ok();
                (concept, clarity)
            }
            _ => {
                (
                    None,
                    Some(self.ai_client.improve_clarity_with_llama(&content).await?)
                )
            }
        };

        Ok(ProcessingResult {
            id,
            file_path: path.to_path_buf(),
            timestamp: Utc::now(),
            conceptualization,
            clarity_improvement,
            processing_time_ms: start.elapsed().as_millis() as u64,
            status: ProcessingStatus::Completed,
        })
    }

    async fn save_result(&self, result: &ProcessingResult) -> Result<()> {
        let output_dir = expand_path(&self.config.processing.output_directory);
        fs::create_dir_all(&output_dir)?;

        let result_filename = format!(
            "{}_{}.json",
            result.file_path.file_name().unwrap().to_str().unwrap(),
            result.id
        );
        let result_path = output_dir.join(result_filename);

        let json = serde_json::to_string_pretty(result)?;
        fs::write(&result_path, json)?;

        // Also save improved text if available
        if let Some(ref clarity) = result.clarity_improvement {
            let improved_filename = format!(
                "{}_{}_improved.txt",
                result.file_path.file_name().unwrap().to_str().unwrap(),
                result.id
            );
            let improved_path = output_dir.join(improved_filename);
            fs::write(&improved_path, &clarity.improved_text)?;
        }

        Ok(())
    }
}
