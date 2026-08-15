<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo-dark.svg">
    <img src="assets/logo-light.svg" alt="Goglz" width="96" height="96">
  </picture>
</p>

<h1 align="center">Goglz</h1>

Goglz is an intelligent document monitoring daemon written in Rust. It watches specified directories for file changes, automatically analyzes documents using state-of-the-art AI models, and generates improved versions with enhanced clarity.

## Overview

Goglz operates as a background service that continuously monitors your document repositories. When files are created or modified, it automatically:

1. **Conceptualizes** content using GPT OSS 120B model to extract summaries, key concepts, and themes
2. **Improves clarity** using LLaMA models via Groq API for fast, high-quality text enhancement
3. **Structures output** in a organized format with both JSON metadata and improved text versions

## Key Features

### Intelligent Monitoring
- **Multi-directory support**: Monitor multiple directories simultaneously with individual configuration
- **Pattern matching**: Filter files by extension patterns (e.g., `*.txt`, `*.md`, `*.rst`)
- **Recursive watching**: Include subdirectories automatically when enabled
- **Debouncing**: Smart delay system prevents processing of actively edited files

### AI-Powered Analysis
- **GPT OSS 120B**: Deep conceptualization for comprehensive document understanding
- **GPT OSS 20B**: Specialized clarity improvement for focused text enhancement
- **LLaMA via Groq**: Fast inference using state-of-the-art open-source models
- **Structured output**: Consistent JSON format with summary, concepts, themes, and clarity scores

### Production-Ready Daemon
- **Background operation**: Runs as a system daemon with proper PID management
- **Robust error handling**: Graceful degradation and detailed error reporting
- **Configurable processing**: Control file size limits, batch sizes, and timing parameters
- **Clean shutdown**: Proper signal handling for clean daemon termination

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/elci-group/goglz.git
cd goglz

# Build in release mode
cargo build --release

# The binary will be at target/release/goglz
```

### Using Baby

```bash
baby install goglz
```

## Configuration

### Initial Setup

Goglz uses a TOML configuration file located at `~/.goglz`. Initialize it with:

```bash
goglz init-config
```

### Configuration Structure

```toml
# Directories to monitor
[[directories]]
path = "~/Documents"
file_patterns = ["*.txt", "*.md", "*.rst"]
recursive = true

[[directories]]
path = "~/Notes"
file_patterns = ["*.md"]
recursive = true

# GPT OSS Configuration (for conceptualization)
[gpt_oss]
api_endpoint = "https://api.gpt-oss.com/v1"
api_key = "your-gpt-oss-api-key"
model_120b = "gpt-oss-120b"
model_20b = "gpt-oss-20b"

# Groq Configuration (for LLaMA inference)
[groq]
api_key = "your-groq-api-key"
model = "llama3-70b-8192"
api_endpoint = "https://api.groq.com/openai/v1"

# Processing settings
[processing]
output_directory = "~/.goglz_output"
max_file_size_mb = 10
batch_size = 5
debounce_interval_ms = 2000
```

### Configuration Options

#### Directories
- `path`: Directory path to monitor (supports `~` expansion)
- `file_patterns`: Array of glob patterns for file matching
- `recursive`: Whether to watch subdirectories

#### GPT OSS
- `api_endpoint`: Base URL for GPT OSS API
- `api_key`: Your GPT OSS API key
- `model_120b`: Model identifier for 120B parameter model
- `model_20b`: Model identifier for 20B parameter model

#### Groq
- `api_key`: Your Groq API key
- `model`: LLaMA model to use (e.g., `llama3-70b-8192`)
- `api_endpoint`: Base URL for Groq API

#### Processing
- `output_directory`: Where to save processed results
- `max_file_size_mb`: Maximum file size to process (in MB)
- `batch_size`: Number of files to process simultaneously
- `debounce_interval_ms`: Delay before processing changed files (milliseconds)

## Usage

### Starting the Daemon

#### Background Mode (Recommended)
```bash
goglz start
```

#### Foreground Mode (Debugging)
```bash
goglz start --foreground
```

### Managing the Daemon

```bash
# Check if daemon is running
goglz status

# Stop the daemon
goglz stop

# Reinitialize configuration
goglz init-config
```

## Processing Pipeline

### Workflow

1. **File Detection**: Monitor detects file creation or modification events
2. **Debouncing**: Waits for configured interval to avoid processing active edits
3. **Content Analysis**: 
   - Reads file content (subject to size limits)
   - Sends to GPT OSS 120B for conceptualization
   - Sends to LLaMA via Groq for clarity improvement
4. **Result Generation**: Creates structured output with metadata and improved text
5. **Output Storage**: Saves results to configured output directory

### Output Format

Each processed file generates two outputs:

#### JSON Result (`<filename>_<uuid>.json`)
```json
{
  "id": "unique-identifier",
  "file_path": "/path/to/original/file",
  "timestamp": "2024-01-01T00:00:00Z",
  "conceptualization": {
    "summary": "Concise document summary",
    "key_concepts": ["concept1", "concept2"],
    "themes": ["theme1", "theme2"],
    "clarity_score": 0.75
  },
  "clarity_improvement": {
    "original_text": "Original content",
    "improved_text": "Improved content",
    "changes_made": ["Change1", "Change2"],
    "clarity_improvement": 1.25
  },
  "processing_time_ms": 1500,
  "status": "Completed"
}
```

#### Improved Text (`<filename>_<uuid>_improved.txt`)
Contains only the improved text version for easy reference.

## Architecture

### Module Structure

- **`config.rs`**: Configuration loading, parsing, and validation
- **`monitor.rs`**: Directory watching using `notify` crate with event debouncing
- **`ai_client.rs`**: HTTP clients for GPT OSS and Groq APIs with error handling
- **`processor.rs`**: Document processing pipeline with batching and concurrency
- **`error.rs`**: Comprehensive error types and handling
- **`main.rs`**: CLI interface, daemon orchestration, and signal handling

### Technology Stack

- **Runtime**: Tokio async runtime
- **File Watching**: `notify` crate for cross-platform file system events
- **HTTP Client**: `reqwest` for API communication
- **Serialization**: `serde` and `serde_json` for data handling
- **Configuration**: `toml` for config file parsing
- **CLI**: `clap` for command-line interface
- **Daemonization**: `daemonize` crate for background operation

## Requirements

- **Rust**: 1.70 or later
- **API Keys**: Valid credentials for GPT OSS and Groq
- **Operating System**: Linux (daemonization uses libc)
- **Disk Space**: Sufficient space for processed output files

## Development

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test
```

### Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

## Troubleshooting

### Common Issues

**Daemon won't start**
- Check configuration file syntax: `goglz init-config`
- Verify API keys are set correctly
- Check log files: `/tmp/goglz.stdout` and `/tmp/goglz.stderr`

**Files not being processed**
- Verify directory paths are correct
- Check file patterns match your files
- Ensure file size is under the configured limit
- Check API quotas and rate limits

**API errors**
- Verify API keys are valid and active
- Check API endpoints are accessible
- Review rate limits and quotas
- Check network connectivity

## License

MIT License - see LICENSE file for details

## Contributing

Contributions are welcome! Please feel free to submit pull requests or open issues for bugs and feature requests.

## Acknowledgments

Built with:
- [Tokio](https://tokio.rs/) for async runtime
- [notify](https://github.com/notify-rs/notify) for file system monitoring
- [reqwest](https://github.com/seanmonstar/reqwest) for HTTP clients
- [clap](https://github.com/clap-rs/clap) for CLI parsing
