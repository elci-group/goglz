// SPDX-License-Identifier: MIT
//! Translation generation for revised documents.

use crate::anti_hunk::apply_anti_hunking;
use crate::error::{GoglzError, Result};
use std::fs;
use std::path::Path;
use tracing::info;

impl crate::revise::ReviseProcessor {
    pub(crate) async fn generate_translations(
        &self,
        original_path: &Path,
        content: &str,
        languages: &[String],
    ) -> Result<()> {
        // Perform parallel translation
        let translations = self
            .ai_client
            .translate_document_parallel(content, languages)
            .await?;

        info!("Successfully generated {} translations", translations.len());

        // Save each translation with language-specific filename
        for (lang_name, translated_content) in translations {
            let lang_config = self
                .config
                .languages
                .iter()
                .find(|l| l.name == lang_name)
                .ok_or_else(|| {
                    GoglzError::ProcessingFailed(format!(
                        "Language config not found: {}",
                        lang_name
                    ))
                })?;

            let output_path = self.generate_language_output_path(original_path, lang_config)?;

            let translated_content =
                apply_anti_hunking(&translated_content, &self.config.anti_hunking);

            fs::write(&output_path, translated_content).map_err(|e| {
                GoglzError::ProcessingFailed(format!("Failed to write translation: {}", e))
            })?;

            info!("Saved {} translation: {:?}", lang_name, output_path);
        }

        Ok(())
    }
}
