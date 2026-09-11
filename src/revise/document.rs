// SPDX-License-Identifier: MIT
//! Per-document revision and translation orchestration.

use crate::anti_hunk::apply_anti_hunking;
use crate::error::{GoglzError, Result};
use crate::processor::{ProcessingResult, ProcessingStatus};
use crate::revise::{assets, prompt};
use chrono::Utc;
use std::fs;
use std::path::Path;
use tracing::info;

impl crate::revise::ReviseProcessor {
    pub(crate) async fn revise_document(&self, path: &Path) -> Result<ProcessingResult> {
        info!("Revising document: {:?}", path);

        let content = fs::read_to_string(path)
            .map_err(|e| GoglzError::ProcessingFailed(format!("Failed to read file: {}", e)))?;

        // Load context from assets
        let asset_context = assets::load_asset_context(&self.project_root, path, &self.config);

        // Build revision prompt based on configuration
        let prompt = prompt::build_revision_prompt(&self.config, &content, &asset_context);

        // Call AI to revise the document (base language revision)
        let improved_content = self.ai_client.revise_document(&prompt).await?;

        // Ventilate long markdown sections so downstream tooling (e.g. Ferret)
        // does not flag the amended document as a "large hunk".
        let improved_content = apply_anti_hunking(&improved_content, &self.config.anti_hunking);

        // Create backup of original
        self.create_backup(path, &content)?;

        // Write revised content
        fs::write(path, &improved_content).map_err(|e| {
            GoglzError::ProcessingFailed(format!("Failed to write revised file: {}", e))
        })?;

        // Handle multi-language generation if enabled
        let enabled_languages: Vec<String> = self
            .config
            .languages
            .iter()
            .filter(|lang| lang.enabled)
            .map(|lang| lang.name.clone())
            .collect();

        if !enabled_languages.is_empty() {
            info!(
                "Generating translations for {} languages",
                enabled_languages.len()
            );
            self.generate_translations(path, &improved_content, &enabled_languages)
                .await?;
        }

        // Return processing result
        Ok(ProcessingResult {
            id: uuid::Uuid::new_v4().to_string(),
            file_path: path.to_path_buf(),
            timestamp: Utc::now(),
            conceptualization: None,
            clarity_improvement: None, // Could be enhanced to track changes
            processing_time_ms: 0,     // Could be enhanced to track timing
            status: ProcessingStatus::Completed,
        })
    }
}
