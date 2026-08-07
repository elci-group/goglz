use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub directories: Vec<MonitoredDirectory>,
    pub gpt_oss: GptOssConfig,
    pub groq: GroqConfig,
    pub processing: ProcessingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoredDirectory {
    pub path: PathBuf,
    pub file_patterns: Vec<String>,
    pub recursive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptOssConfig {
    pub api_endpoint: String,
    pub api_key: String,
    pub model_120b: String,
    pub model_20b: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroqConfig {
    pub api_key: String,
    pub model: String,
    pub api_endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    pub output_directory: PathBuf,
    pub max_file_size_mb: u64,
    pub batch_size: usize,
    pub debounce_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviseConfig {
    pub purpose: String,
    pub scope: String,
    pub writing_style: WritingStyle,
    pub formatting_rules: FormattingRules,
    pub global_assets: Vec<AssetReference>,
    pub local_assets: Vec<AssetReference>,
    pub languages: Vec<LanguageConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageConfig {
    pub code: String,        // ISO language code (e.g., "en", "es", "zh")
    pub name: String,        // Full language name (e.g., "English", "Spanish", "Chinese")
    pub enabled: bool,      // Whether this language is enabled for translation
    pub output_pattern: String, // Output filename pattern (e.g., "{filename}_{lang}.{ext}")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritingStyle {
    pub tone: String,
    pub voice: String,
    pub audience: String,
    pub guidelines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormattingRules {
    pub headings: bool,
    pub bullet_points: bool,
    pub numbered_lists: bool,
    pub code_blocks: bool,
    pub max_line_length: Option<usize>,
    pub custom_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetReference {
    pub path: PathBuf,
    pub asset_type: String,
    pub description: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            directories: vec![],
            gpt_oss: GptOssConfig {
                api_endpoint: "https://api.gpt-oss.com/v1".to_string(),
                api_key: std::env::var("GPT_OSS_API_KEY").unwrap_or_default(),
                model_120b: "gpt-oss-120b".to_string(),
                model_20b: "gpt-oss-20b".to_string(),
            },
            groq: GroqConfig {
                api_key: std::env::var("GROQ_API_KEY").unwrap_or_default(),
                model: "llama3-70b-8192".to_string(),
                api_endpoint: "https://api.groq.com/openai/v1".to_string(),
            },
            processing: ProcessingConfig {
                output_directory: PathBuf::from("~/.goglz_output"),
                max_file_size_mb: 10,
                batch_size: 5,
                debounce_interval_ms: 2000,
            },
        }
    }
}

pub fn load_config() -> Result<Config> {
    let config_path = dirs::home_dir()
        .context("Could not determine home directory")?
        .join(".goglz");

    if !config_path.exists() {
        tracing::warn!("Config file not found at {:?}, using defaults", config_path);
        return Ok(Config::default());
    }

    let content = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config file at {:?}", config_path))?;

    let config: Config = toml::from_str(&content)
        .with_context(|| format!("Failed to parse TOML config from {:?}", config_path))?;

    tracing::info!("Loaded configuration from {:?}", config_path);
    Ok(config)
}

pub fn expand_path(path: &PathBuf) -> PathBuf {
    if path.starts_with("~") {
        if let Some(home) = dirs::home_dir() {
            return home.join(path.strip_prefix("~").unwrap());
        }
    }
    path.clone()
}

pub fn load_revise_config(project_root: &Path) -> Result<ReviseConfig> {
    let config_path = project_root.join("goglz.yaml");
    
    if !config_path.exists() {
        tracing::warn!("goglz.yaml not found at {:?}, using defaults", config_path);
        return Ok(default_revise_config());
    }

    let content = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read goglz.yaml at {:?}", config_path))?;

    let config: ReviseConfig = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse YAML config from {:?}", config_path))?;

    tracing::info!("Loaded revise configuration from {:?}", config_path);
    Ok(config)
}

fn default_revise_config() -> ReviseConfig {
    ReviseConfig {
        purpose: "Improve document clarity and readability".to_string(),
        scope: "All documentation files".to_string(),
        writing_style: WritingStyle {
            tone: "Professional and clear".to_string(),
            voice: "Objective and informative".to_string(),
            audience: "General technical audience".to_string(),
            guidelines: vec![
                "Use active voice".to_string(),
                "Avoid jargon when possible".to_string(),
                "Be concise but comprehensive".to_string(),
            ],
        },
        formatting_rules: FormattingRules {
            headings: true,
            bullet_points: true,
            numbered_lists: true,
            code_blocks: true,
            max_line_length: Some(80),
            custom_rules: vec![],
        },
        global_assets: vec![],
        local_assets: vec![],
        languages: vec![],
    }
}
