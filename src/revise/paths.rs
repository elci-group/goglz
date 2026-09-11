// SPDX-License-Identifier: MIT
//! Translation output path helpers for `goglz revise`.

use crate::config::LanguageConfig;
use crate::error::{GoglzError, Result};
use std::path::{Path, PathBuf};

impl crate::revise::ReviseProcessor {
    pub fn generate_language_output_path(
        &self,
        original_path: &Path,
        lang_config: &LanguageConfig,
    ) -> Result<PathBuf> {
        let filename = original_path
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or_else(|| GoglzError::ProcessingFailed("Invalid filename".to_string()))?;

        let extension = original_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("md");

        let stem = filename.trim_end_matches(&format!(".{}", extension));

        // Apply the output pattern from config
        let output_filename = lang_config
            .output_pattern
            .replace("{filename}", stem)
            .replace("{lang}", &lang_config.code.to_lowercase())
            .replace("{ext}", extension);

        let output_path = original_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(output_filename);

        Ok(output_path)
    }
}
