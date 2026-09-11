// SPDX-License-Identifier: MIT
//! High-level orchestration for the `revise` pipeline.

use crate::error::Result;
use crate::processor::ProcessingResult;
use tracing::{error, info};

impl crate::revise::ReviseProcessor {
    pub async fn run(&self) -> Result<Vec<ProcessingResult>> {
        info!(
            "Starting revision process for directory: {:?}",
            self.target_directory
        );

        let documents = self.discover_documents();
        info!("Found {} documents to revise", documents.len());

        let mut results = Vec::new();

        for document_path in documents {
            match self.revise_document(&document_path).await {
                Ok(result) => {
                    info!("Successfully revised: {:?}", document_path);
                    results.push(result);
                }
                Err(e) => {
                    error!("Failed to revise {:?}: {}", document_path, e);
                    // Continue with other documents even if one fails
                }
            }
        }

        info!("Revision complete. Processed {} documents.", results.len());
        Ok(results)
    }
}
