//! Shared helpers for goglz integration tests.
//!
//! Nothing in here performs real network I/O. Anywhere an `AiClient` is
//! needed, tests point it at `DEAD_ENDPOINT` (a loopback address nothing is
//! listening on) so the underlying `reqwest` call fails fast and
//! deterministically with a connection-refused error, instead of hitting a
//! real API. That lets us exercise real error-propagation / rollback-safety
//! code paths without mocking the AI client itself.
#![allow(dead_code)]

use goglz::ai_client::AiClient;
use goglz::config::{
    AssetReference, Config, FormattingRules, GptOssConfig, GroqConfig, LanguageConfig,
    MonitoredDirectory, ProcessingConfig, ReviseConfig, WritingStyle,
};
use std::path::PathBuf;

/// A loopback address with nothing listening on it. Connection attempts fail
/// immediately (`ECONNREFUSED`) rather than timing out, and never touch the
/// network beyond the local machine.
pub const DEAD_ENDPOINT: &str = "http://127.0.0.1:1";

/// Build a `Config` for tests. `output_directory` should be a temp dir so
/// tests never write into the real `~/.goglz_output`.
pub fn test_config(output_directory: PathBuf, max_file_size_mb: u64) -> Config {
    Config {
        directories: vec![],
        gpt_oss: GptOssConfig {
            api_endpoint: DEAD_ENDPOINT.to_string(),
            api_key: "test-key".to_string(),
            model_120b: "gpt-oss-120b".to_string(),
            model_20b: "gpt-oss-20b".to_string(),
        },
        groq: GroqConfig {
            api_key: "test-key".to_string(),
            model: "llama3-70b-8192".to_string(),
            api_endpoint: DEAD_ENDPOINT.to_string(),
        },
        processing: ProcessingConfig {
            output_directory,
            max_file_size_mb,
            batch_size: 5,
            debounce_interval_ms: 2000,
        },
    }
}

pub fn monitored_directory(path: PathBuf, file_patterns: Vec<&str>, recursive: bool) -> MonitoredDirectory {
    MonitoredDirectory {
        path,
        file_patterns: file_patterns.into_iter().map(String::from).collect(),
        recursive,
    }
}

pub fn test_ai_client(config: &Config) -> AiClient {
    AiClient::new(config)
}

/// A minimal `ReviseConfig`, equivalent in shape to the crate's own
/// (private) `default_revise_config()`.
pub fn minimal_revise_config() -> ReviseConfig {
    ReviseConfig {
        purpose: "Improve document clarity and readability".to_string(),
        scope: "All documentation files".to_string(),
        writing_style: WritingStyle {
            tone: "Professional and clear".to_string(),
            voice: "Objective and informative".to_string(),
            audience: "General technical audience".to_string(),
            guidelines: vec!["Use active voice".to_string()],
        },
        formatting_rules: FormattingRules {
            headings: true,
            bullet_points: true,
            numbered_lists: true,
            code_blocks: true,
            max_line_length: Some(80),
            custom_rules: vec![],
        },
        global_assets: Vec::<AssetReference>::new(),
        local_assets: Vec::<AssetReference>::new(),
        languages: Vec::<LanguageConfig>::new(),
    }
}

pub fn language_config(code: &str, name: &str, enabled: bool, output_pattern: &str) -> LanguageConfig {
    LanguageConfig {
        code: code.to_string(),
        name: name.to_string(),
        enabled,
        output_pattern: output_pattern.to_string(),
    }
}
