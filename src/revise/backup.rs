// SPDX-License-Identifier: MIT
//! Backup creation for `goglz revise`.

use crate::error::{GoglzError, Result};
use std::fs;
use std::path::Path;
use tracing::info;

impl crate::revise::ReviseProcessor {
    pub fn create_backup(&self, path: &Path, content: &str) -> Result<()> {
        let backup_ext = match path.extension().and_then(|e| e.to_str()) {
            Some(ext) => format!("{}.backup", ext),
            None => "backup".to_string(),
        };
        let backup_path = path.with_extension(&backup_ext);

        fs::write(&backup_path, content)
            .map_err(|e| GoglzError::ProcessingFailed(format!("Failed to create backup: {}", e)))?;

        info!("Created backup at: {:?}", backup_path);
        Ok(())
    }
}
