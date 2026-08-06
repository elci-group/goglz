# Goglz

A Rust daemon that monitors directories, conceptualizes documents, and improves clarity using AI models.

## Features

- **Directory Monitoring**: Watch multiple directories for file changes using the `notify` crate
- **AI-Powered Analysis**: 
  - GPT OSS 120B for deep document conceptualization
  - GPT OSS 20B for clarity improvement
  - LLaMA models via Groq API for fast inference
- **Daemon Mode**: Runs as a background service with PID management
- **Configurable**: TOML-based configuration file at `~/.goglz`

## Installation

1. Clone the repository and build:
```bash
cargo build --release
```

2. The binary will be available at `target/release/goglz`

## Configuration

Initialize the configuration file:
```bash
./target/release/goglz init-config
```

This creates `~/.goglz` with default settings. Edit it to add your API keys and configure directories:

```toml
# Directories to monitor
[[directories]]
path = "~/Documents"
file_patterns = ["*.txt", "*.md", "*.rst"]
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

## Usage

### Start the daemon

Run in the background (daemon mode):
```bash
./target/release/goglz start
```

Run in foreground (for debugging):
```bash
./target/release/goglz start --foreground
```

### Check status
```bash
./target/release/goglz status
```

### Stop the daemon
```bash
./target/release/goglz stop
```

## How It Works

1. **Monitoring**: The daemon watches configured directories for file changes (create/modify events)
2. **Debouncing**: Changes are debounced to avoid processing files that are being actively edited
3. **Processing**: For each detected file:
   - Reads the file content
   - Sends to GPT OSS 120B for conceptualization (summary, key concepts, themes, clarity score)
   - Sends to LLaMA via Groq for clarity improvement
   - Saves results to the output directory
4. **Output**: Results are saved as JSON files and improved text versions in `~/.goglz_output`

## Output Format

Each processed file generates:
- `<filename>_<uuid>.json`: Full processing result with conceptualization and clarity improvement
- `<filename>_<uuid>_improved.txt`: The improved text version

## Architecture

- `config.rs`: Configuration loading and parsing
- `monitor.rs`: Directory watching using `notify`
- `ai_client.rs`: API clients for GPT OSS and Groq
- `processor.rs`: Document processing pipeline
- `main.rs`: CLI interface and daemon orchestration

## Requirements

- Rust 1.70 or later
- Valid API keys for GPT OSS and Groq
- Linux (daemonization uses libc)

## License

MIT
