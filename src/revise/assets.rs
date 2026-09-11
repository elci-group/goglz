// SPDX-License-Identifier: MIT
//! Asset context loading for `goglz revise`.

use crate::config::ReviseConfig;
use std::fs;
use std::path::Path;
use tracing::warn;

/// Load global and local asset files referenced by the revision config.
///
/// Missing or unreadable assets are logged and skipped rather than failing the
/// whole revision, so this function returns the accumulated context directly.
pub(crate) fn load_asset_context(
    project_root: &Path,
    document_path: &Path,
    config: &ReviseConfig,
) -> String {
    let mut context = String::new();

    // Load global assets
    for asset in &config.global_assets {
        let asset_path = project_root.join(&asset.path);
        if asset_path.exists() {
            if let Ok(content) = fs::read_to_string(&asset_path) {
                context.push_str(&format!(
                    "\n--- Global Asset: {} ({}) ---\n{}\n",
                    asset.description, asset.asset_type, content
                ));
            }
        } else {
            warn!("Global asset not found: {:?}", asset_path);
        }
    }

    // Load local assets (relative to document)
    for asset in &config.local_assets {
        let asset_path = document_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(&asset.path);

        if asset_path.exists() {
            if let Ok(content) = fs::read_to_string(&asset_path) {
                context.push_str(&format!(
                    "\n--- Local Asset: {} ({}) ---\n{}\n",
                    asset.description, asset.asset_type, content
                ));
            }
        } else {
            warn!("Local asset not found: {:?}", asset_path);
        }
    }

    context
}
