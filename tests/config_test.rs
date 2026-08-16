//! Config parsing and `~` path expansion.
//!
//! `load_config()` hardcodes the path `~/.goglz` (the real user's home
//! directory) rather than accepting a path parameter, so it can't safely be
//! exercised end-to-end from a test without risking reads/writes against a
//! real developer machine's actual config file. Instead these tests drive
//! the same deserialization logic (`toml::from_str::<Config>` /
//! `serde_yaml::from_str::<ReviseConfig>`) that `load_config` /
//! `load_revise_config` call internally - that's the part of the pipeline
//! actually responsible for "valid TOML loads, malformed TOML errors
//! cleanly." `load_revise_config`, by contrast, *does* take a directory
//! argument, so it's exercised directly against a temp dir below.

use goglz::config::{expand_path, load_revise_config, Config, ReviseConfig};
use std::path::PathBuf;

const VALID_TOML: &str = r#"
[[directories]]
path = "~/Documents"
file_patterns = ["*.txt", "*.md"]
recursive = true

[gpt_oss]
api_endpoint = "https://api.gpt-oss.com/v1"
api_key = "key-a"
model_120b = "gpt-oss-120b"
model_20b = "gpt-oss-20b"

[groq]
api_key = "key-b"
model = "llama3-70b-8192"
api_endpoint = "https://api.groq.com/openai/v1"

[processing]
output_directory = "~/.goglz_output"
max_file_size_mb = 25
batch_size = 7
debounce_interval_ms = 1500
"#;

#[test]
fn valid_toml_parses_with_expected_fields() {
    let config: Config = toml::from_str(VALID_TOML).expect("valid TOML must parse");

    assert_eq!(config.directories.len(), 1);
    assert_eq!(config.directories[0].path, PathBuf::from("~/Documents"));
    assert_eq!(
        config.directories[0].file_patterns,
        vec!["*.txt".to_string(), "*.md".to_string()]
    );
    assert!(config.directories[0].recursive);

    assert_eq!(config.gpt_oss.api_key, "key-a");
    assert_eq!(config.groq.api_key, "key-b");
    assert_eq!(config.processing.max_file_size_mb, 25);
    assert_eq!(config.processing.batch_size, 7);
    assert_eq!(config.processing.debounce_interval_ms, 1500);
}

#[test]
fn config_default_has_documented_defaults() {
    let config = Config::default();

    assert!(config.directories.is_empty());
    assert_eq!(config.processing.max_file_size_mb, 10);
    assert_eq!(config.processing.batch_size, 5);
    assert_eq!(config.processing.debounce_interval_ms, 2000);
    assert_eq!(
        config.processing.output_directory,
        PathBuf::from("~/.goglz_output")
    );
    assert_eq!(config.gpt_oss.model_120b, "gpt-oss-120b");
    assert_eq!(config.groq.model, "llama3-70b-8192");
}

#[test]
fn malformed_toml_is_a_clean_error_not_a_panic() {
    // Missing closing bracket / invalid syntax.
    let broken = "[[directories]\npath = \"~/Documents\"";
    let result: Result<Config, _> = toml::from_str(broken);
    assert!(result.is_err(), "malformed TOML must not parse");
}

#[test]
fn toml_missing_required_fields_is_a_clean_error_not_a_panic() {
    // Syntactically valid TOML, but missing required sections/fields for `Config`.
    let incomplete = r#"
[[directories]]
path = "~/Documents"
file_patterns = ["*.md"]
recursive = true
"#;
    let result: Result<Config, _> = toml::from_str(incomplete);
    assert!(
        result.is_err(),
        "TOML missing gpt_oss/groq/processing sections must not silently succeed"
    );
}

#[test]
fn empty_toml_is_a_clean_error_not_a_panic() {
    let result: Result<Config, _> = toml::from_str("");
    assert!(result.is_err());
}

#[test]
fn valid_yaml_revise_config_loads_from_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let yaml = r#"
purpose: "Keep docs accurate"
scope: "docs/"
writing_style:
  tone: "Friendly"
  voice: "Active"
  audience: "Developers"
  guidelines:
    - "Be concise"
formatting_rules:
  headings: true
  bullet_points: false
  numbered_lists: true
  code_blocks: true
  max_line_length: 100
  custom_rules: []
global_assets: []
local_assets: []
languages:
  - code: "es"
    name: "Spanish"
    enabled: true
    output_pattern: "{filename}_{lang}.{ext}"
"#;
    std::fs::write(dir.path().join("goglz.yaml"), yaml).unwrap();

    let config: ReviseConfig = load_revise_config(dir.path()).expect("valid YAML must load");
    assert_eq!(config.purpose, "Keep docs accurate");
    assert_eq!(config.writing_style.tone, "Friendly");
    assert_eq!(config.formatting_rules.max_line_length, Some(100));
    assert_eq!(config.languages.len(), 1);
    assert!(config.languages[0].enabled);
}

#[test]
fn missing_goglz_yaml_falls_back_to_defaults_without_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    // No goglz.yaml written - load_revise_config must not error.
    let config = load_revise_config(dir.path()).expect("missing file must fall back, not error");
    assert!(!config.purpose.is_empty());
    assert!(config.languages.is_empty());
}

#[test]
fn malformed_goglz_yaml_is_a_clean_error_not_a_panic() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("goglz.yaml"), "purpose: [this is not valid: yaml").unwrap();

    let result = load_revise_config(dir.path());
    assert!(result.is_err(), "malformed YAML must not parse");
}

#[test]
fn expand_path_bare_tilde_is_home_dir() {
    let home = dirs::home_dir().expect("home dir must resolve in test environment");
    assert_eq!(expand_path(&PathBuf::from("~")), home);
}

#[test]
fn expand_path_tilde_slash_is_home_dir() {
    let home = dirs::home_dir().expect("home dir must resolve in test environment");
    assert_eq!(expand_path(&PathBuf::from("~/")), home);
}

#[test]
fn expand_path_tilde_with_subpath_joins_home() {
    let home = dirs::home_dir().expect("home dir must resolve in test environment");
    assert_eq!(
        expand_path(&PathBuf::from("~/Documents/notes.md")),
        home.join("Documents/notes.md")
    );
}

#[test]
fn expand_path_without_tilde_is_unchanged() {
    assert_eq!(
        expand_path(&PathBuf::from("/absolute/path")),
        PathBuf::from("/absolute/path")
    );
    assert_eq!(
        expand_path(&PathBuf::from("relative/path")),
        PathBuf::from("relative/path")
    );
}

#[test]
fn expand_path_username_like_prefix_is_not_mistaken_for_home() {
    // "~foo" is a single path component ("~foo"), not the `~` component
    // followed by "foo" - expand_path must not expand it, since it does not
    // actually resolve to the current user's home directory.
    assert_eq!(
        expand_path(&PathBuf::from("~foo/bar")),
        PathBuf::from("~foo/bar")
    );
}
