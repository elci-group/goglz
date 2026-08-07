use crate::ai_client::AiClient;
use crate::config::ReviseConfig;
use crate::error::{GoglzError, Result};
use crate::processor::ProcessingResult;
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn, error};
use walkdir::WalkDir;

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

    pub async fn run(&self) -> Result<Vec<ProcessingResult>> {
        info!("Starting revision process for directory: {:?}", self.target_directory);
        
        let documents = self.discover_documents()?;
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

    fn discover_documents(&self) -> Result<Vec<PathBuf>> {
        let mut documents = Vec::new();
        
        // Common documentation file extensions
        let extensions = vec![
            "md", "txt", "rst", "asciidoc", "adoc", "doc", "docx",
        ];

        for entry in WalkDir::new(&self.target_directory)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            
            // Skip directories and hidden files
            if path.is_dir() || path.file_name()
                .map(|f| f.to_string_lossy().starts_with('.'))
                .unwrap_or(false) {
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
        Ok(documents)
    }

    async fn revise_document(&self, path: &Path) -> Result<ProcessingResult> {
        info!("Revising document: {:?}", path);
        
        let content = fs::read_to_string(path)
            .map_err(|e| GoglzError::ProcessingFailed(format!("Failed to read file: {}", e)))?;

        // Load context from assets
        let asset_context = self.load_asset_context(path)?;

        // Build revision prompt based on configuration
        let prompt = self.build_revision_prompt(&content, &asset_context);

        // Call AI to revise the document
        let improved_content = self.ai_client.revise_document(&prompt).await?;

        // Create backup of original
        self.create_backup(path, &content)?;

        // Write revised content
        fs::write(path, &improved_content)
            .map_err(|e| GoglzError::ProcessingFailed(format!("Failed to write revised file: {}", e)))?;

        // Return processing result
        Ok(ProcessingResult {
            id: uuid::Uuid::new_v4().to_string(),
            file_path: path.to_path_buf(),
            timestamp: Utc::now(),
            conceptualization: None,
            clarity_improvement: None, // Could be enhanced to track changes
            processing_time_ms: 0, // Could be enhanced to track timing
            status: crate::processor::ProcessingStatus::Completed,
        })
    }

    fn load_asset_context(&self, document_path: &Path) -> Result<String> {
        let mut context = String::new();

        // Load global assets
        for asset in &self.config.global_assets {
            let asset_path = self.project_root.join(&asset.path);
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
        for asset in &self.config.local_assets {
            let asset_path = document_path.parent()
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

        Ok(context)
    }

    fn build_revision_prompt(&self, content: &str, asset_context: &str) -> String {
        let mut prompt = String::new();

        // Add purpose and scope
        prompt.push_str(&format!("Purpose: {}\n", self.config.purpose));
        prompt.push_str(&format!("Scope: {}\n\n", self.config.scope));

        // Add writing style guidelines
        prompt.push_str("Writing Style:\n");
        prompt.push_str(&format!("- Tone: {}\n", self.config.writing_style.tone));
        prompt.push_str(&format!("- Voice: {}\n", self.config.writing_style.voice));
        prompt.push_str(&format!("- Audience: {}\n", self.config.writing_style.audience));
        prompt.push_str("Guidelines:\n");
        for guideline in &self.config.writing_style.guidelines {
            prompt.push_str(&format!("  - {}\n", guideline));
        }
        prompt.push_str("\n");

        // Add formatting rules
        prompt.push_str("Formatting Rules:\n");
        if self.config.formatting_rules.headings {
            prompt.push_str("  - Use clear, hierarchical headings\n");
        }
        if self.config.formatting_rules.bullet_points {
            prompt.push_str("  - Use bullet points for lists\n");
        }
        if self.config.formatting_rules.numbered_lists {
            prompt.push_str("  - Use numbered lists for sequential items\n");
        }
        if self.config.formatting_rules.code_blocks {
            prompt.push_str("  - Use code blocks for technical content\n");
        }
        if let Some(max_len) = self.config.formatting_rules.max_line_length {
            prompt.push_str(&format!("  - Maximum line length: {} characters\n", max_len));
        }
        for custom_rule in &self.config.formatting_rules.custom_rules {
            prompt.push_str(&format!("  - {}\n", custom_rule));
        }
        prompt.push_str("\n");

        // Add asset context if available
        if !asset_context.is_empty() {
            prompt.push_str("Reference Assets:\n");
            prompt.push_str(asset_context);
            prompt.push_str("\n");
        }

        // Add the document content
        prompt.push_str("Original Document:\n");
        prompt.push_str(content);
        prompt.push_str("\n\n");

        // Add instruction
        prompt.push_str("Please revise the document according to the above guidelines. ");
        prompt.push_str("Return only the revised document content without any additional commentary or metadata.");

        prompt
    }

    fn create_backup(&self, path: &Path, content: &str) -> Result<()> {
        let backup_path = path.with_extension(format!("{}.backup", 
            path.extension().and_then(|e| e.to_str()).unwrap_or("bak")));
        
        fs::write(&backup_path, content)
            .map_err(|e| GoglzError::ProcessingFailed(format!("Failed to create backup: {}", e)))?;
        
        info!("Created backup at: {:?}", backup_path);
        Ok(())
    }
}