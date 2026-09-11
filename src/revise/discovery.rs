// SPDX-License-Identifier: MIT
//! Document discovery for the `revise` pipeline.

use std::path::PathBuf;
use walkdir::{DirEntry, WalkDir};

/// True for any directory-tree entry below the root whose name starts with a
/// dot (`.git`, `.github`, `.env`, ...). Used with `WalkDir::filter_entry` to
/// prune hidden directories from traversal entirely, not just skip them once
/// yielded (skipping alone does not stop WalkDir from recursing into them).
fn is_hidden(entry: &DirEntry) -> bool {
    entry.depth() > 0
        && entry
            .file_name()
            .to_str()
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
}

impl crate::revise::ReviseProcessor {
    pub fn discover_documents(&self) -> Vec<PathBuf> {
        let mut documents = Vec::new();

        // Common documentation file extensions
        let extensions = ["md", "txt", "rst", "asciidoc", "adoc", "doc", "docx"];

        for entry in WalkDir::new(&self.target_directory)
            .follow_links(true)
            .into_iter()
            // Prune hidden directories (e.g. `.git`, `.github`, `.venv`) from
            // the walk entirely. Without this, WalkDir still descends into them
            // even though the per-entry checks below skip the directory entries
            // themselves - so a non-dotfile like `.github/ISSUE_TEMPLATE/bug.md`
            // would otherwise slip through and get silently overwritten.
            .filter_entry(|e| !is_hidden(e))
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Skip directories and hidden files
            if path.is_dir()
                || path
                    .file_name()
                    .map(|f| f.to_string_lossy().starts_with('.'))
                    .unwrap_or(false)
            {
                continue;
            }

            // Check if file matches any documentation extension
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy();
                if extensions.contains(&ext_str.as_ref()) {
                    documents.push(path.to_path_buf());
                }
            }
        }

        // Sort for consistent processing order
        documents.sort();
        documents
    }
}
