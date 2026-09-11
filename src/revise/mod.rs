// SPDX-License-Identifier: MIT
//! `goglz revise` pipeline: discover documentation, call the AI, and overwrite
//! files in-place after creating a backup copy.

mod assets;
mod backup;
mod discovery;
mod document;
pub(crate) mod paths;
pub(crate) mod prompt;
mod run;
mod translation;

use crate::ai_client::AiClient;
use crate::config::ReviseConfig;
use std::path::PathBuf;

pub struct ReviseProcessor {
    ai_client: AiClient,
    config: ReviseConfig,
    project_root: PathBuf,
    target_directory: PathBuf,
}

impl ReviseProcessor {
    pub fn new(
        ai_client: AiClient,
        config: ReviseConfig,
        project_root: PathBuf,
        target_directory: Option<PathBuf>,
    ) -> Self {
        let target_dir = target_directory.unwrap_or_else(|| project_root.clone());

        Self {
            ai_client,
            config,
            project_root,
            target_directory: target_dir,
        }
    }
}
