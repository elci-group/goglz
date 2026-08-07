---
name: goglz-dev
description: Development tasks for Goglz document monitoring daemon
argument-hint: "<task>"
allowed-tools:
  - read
  - edit
  - grep
  - glob
  - exec
permissions:
  allow:
    - Exec(cargo)
    - Exec(git)
    - Write(src/**)
    - Write(Cargo.toml)
    - Write(README.md)
  ask:
    - Write(.git/**)
---

You are working on Goglz, an intelligent document monitoring daemon written in Rust. The project watches directories for file changes, analyzes documents using AI models (GPT OSS and LLaMA via Groq), and generates improved versions with enhanced clarity.

## Project Structure

- **src/**: Main source code
  - `config.rs`: Configuration loading, parsing, and validation
  - `monitor.rs`: Directory watching using `notify` crate with event debouncing
  - `ai_client.rs`: HTTP clients for GPT OSS and Groq APIs with error handling and parallel translation
  - `processor.rs`: Document processing pipeline with batching and concurrency
  - `revise.rs`: Document revision logic for on-demand document improvement with multi-language support
  - `error.rs`: Comprehensive error types and handling
  - `main.rs`: CLI interface, daemon orchestration, and signal handling
- **Cargo.toml**: Rust dependencies and project configuration
- **README.md**: Project documentation
- **VERSION**: Current version (0.1.1)
- **goglz.yaml**: Project-specific revision configuration (optional)

## Technology Stack

- **Runtime**: Tokio async runtime
- **File Watching**: `notify` crate for cross-platform file system events
- **HTTP Client**: `reqwest` for API communication
- **Serialization**: `serde`, `serde_json`, and `serde_yaml` for data handling
- **Configuration**: `toml` for daemon config, YAML for revision config
- **CLI**: `clap` for command-line interface
- **Daemonization**: `daemonize` crate for background operation
- **File Discovery**: `walkdir` for recursive directory scanning

## Common Development Tasks

When the user asks for development help:

1. **Build the project**: Use `cargo build` for debug or `cargo build --release` for optimized builds
2. **Run tests**: Use `cargo test` for all tests, or `cargo test -- --nocapture` for verbose output
3. **Check code quality**: Run `cargo clippy` for linting and `cargo fmt` for formatting
4. **Run the daemon**: Use `cargo run -- start` for foreground mode or add daemonization for background
5. **Revise documents**: Use `cargo run -- revise` to revise all docs in the current directory, or `cargo run -- revise --directory <path>` for a specific directory
6. **Add dependencies**: Update Cargo.toml with new dependencies following the existing pattern
7. **Debug issues**: Check the error.rs module for error handling patterns, use tracing for logging

## Code Conventions

- Use `anyhow` for general error handling and `thiserror` for custom error types
- Follow Rust 2021 edition patterns
- Use async/await with Tokio runtime
- Implement proper error handling with Result types
- Use tracing for structured logging
- Follow the existing module structure when adding new features

## Configuration

The daemon uses a TOML configuration file at `~/.goglz` with sections for:
- Directories to monitor with file patterns
- GPT OSS API configuration (120B and 20B models)
- Groq API configuration for LLaMA models
- Processing settings (output directory, file size limits, batch size, debouncing)

The revise command uses a YAML configuration file at `goglz.yaml` in the project root with:
- **purpose**: The overall goal of document revision
- **scope**: What documents should be revised
- **writing_style**: Tone, voice, audience, and style guidelines
- **formatting_rules**: Headings, lists, code blocks, line length, custom rules
- **global_assets**: Reference files available to all documents (style guides, templates)
- **local_assets**: Reference files relative to individual documents

## Testing

When adding new features, ensure:
1. Unit tests for individual functions
2. Integration tests for module interactions
3. Error handling is comprehensive
4. Configuration validation is covered
5. Async code handles timeouts and errors properly

## AI API Integration

The project integrates with:
- **GPT OSS**: For conceptualization (120B model) and clarity improvement (20B model)
- **Groq**: For fast LLaMA model inference and document revision
- Both APIs use structured JSON output with proper error handling and rate limiting awareness

## Revise Command

The `goglz revise` command provides on-demand document revision:
- Scans target directory for documentation files (.md, .txt, .rst, .asciidoc, etc.)
- Loads revision guidelines from `goglz.yaml` in project root
- Uses AI to revise documents according to purpose, style, and formatting rules
- Creates backups of original files before revision
- Supports global and local asset references for context

Help the user with any development tasks while following these conventions and patterns.
