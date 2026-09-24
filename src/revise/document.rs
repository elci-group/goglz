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
        let improved_content = unwrap_model_payload(&content, improved_content)?;

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

/// Guard against writing a serialized model response over a document.
///
/// Models sometimes answer with `{"content": "..."}` instead of the bare
/// document. A well-formed wrapper is unwrapped; anything else JSON-shaped
/// (typically a response truncated mid-string) is rejected so the original
/// file is left untouched. Documents that were JSON to begin with pass
/// through unchanged.
pub(crate) fn unwrap_model_payload(original: &str, revised: String) -> Result<String> {
    let trimmed = revised.trim_start();
    if !trimmed.starts_with('{') || original.trim_start().starts_with('{') {
        return Ok(revised);
    }
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(value) => ["content", "text", "document"]
            .iter()
            .find_map(|key| value.get(key).and_then(|v| v.as_str()))
            .map(str::to_string)
            .ok_or_else(|| {
                GoglzError::ProcessingFailed(
                    "model returned a JSON object with no document text; original left unchanged"
                        .into(),
                )
            }),
        Err(e) => Err(GoglzError::ProcessingFailed(format!(
            "model returned malformed JSON instead of a document ({e}); original left unchanged"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::unwrap_model_payload;

    #[test]
    fn plain_markdown_passes_through() {
        let out = unwrap_model_payload("# A", "# B\n".into()).unwrap();
        assert_eq!(out, "# B\n");
    }

    #[test]
    fn content_wrapper_is_unwrapped() {
        let out = unwrap_model_payload("# A", r##"{"content": "# B\nbody"}"##.into()).unwrap();
        assert_eq!(out, "# B\nbody");
    }

    #[test]
    fn truncated_wrapper_is_rejected() {
        assert!(unwrap_model_payload("# A", "{\n  \"content\": \"# B\\n".into()).is_err());
    }

    #[test]
    fn json_documents_are_left_alone() {
        let out = unwrap_model_payload("{\"a\": 1}", "{\"a\": 2}".into()).unwrap();
        assert_eq!(out, "{\"a\": 2}");
    }
}
