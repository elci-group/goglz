mod ai_client;
mod config;
mod error;
mod monitor;
mod processor;

use crate::ai_client::AiClient;
use crate::config::load_config;
use crate::error::Result;
use crate::monitor::DirectoryMonitor;
use crate::processor::DocumentProcessor;
use clap::{Parser, Subcommand};
use daemonize::Daemonize;
use std::fs::File;
use std::io::{Write, BufRead};
use tokio::sync::mpsc;
use tracing::info;
use tracing_subscriber;

#[derive(Parser)]
#[command(name = "goglz")]
#[command(about = "A daemon that monitors directories and improves document clarity using AI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the goglz daemon
    Start {
        /// Run in foreground (do not daemonize)
        #[arg(short, long)]
        foreground: bool,
    },
    /// Stop the goglz daemon
    Stop,
    /// Show the status of the goglz daemon
    Status,
    /// Generate an example configuration file
    InitConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start { foreground } => {
            start_daemon(foreground).await?;
        }
        Commands::Stop => {
            stop_daemon()?;
        }
        Commands::Status => {
            show_status()?;
        }
        Commands::InitConfig => {
            init_config()?;
        }
    }

    Ok(())
}

async fn start_daemon(foreground: bool) -> Result<()> {
    // Load configuration
    let config = load_config()?;
    
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting goglz daemon...");

    if !foreground {
        let stdout = File::create("/tmp/goglz.stdout")?;
        let stderr = File::create("/tmp/goglz.stderr")?;
        let pid_file = "/tmp/goglz.pid";

        let daemonize = Daemonize::new()
            .pid_file(pid_file)
            .chown_pid_file(true)
            .working_directory("/tmp")
            .stdout(stdout)
            .stderr(stderr);

        match daemonize.start() {
            Ok(_) => info!("Daemonized successfully"),
            Err(e) => {
                eprintln!("Failed to daemonize: {}", e);
                return Err(crate::error::GoglzError::Daemonization(e.to_string()));
            }
        }
    }

    // Create channel for file events
    let (event_sender, event_receiver) = mpsc::unbounded_channel();

    // Initialize AI client
    let ai_client = AiClient::new(&config);

    // Start directory monitor
    let monitor = DirectoryMonitor::new(config.clone(), event_sender)?;
    monitor.start().await?;

    // Start document processor
    let mut processor = DocumentProcessor::new(config, ai_client, event_receiver);
    
    info!("Goglz daemon is running and monitoring directories...");
    
    // Run the processor (this blocks indefinitely)
    processor.run().await?;

    Ok(())
}

fn stop_daemon() -> Result<()> {
    let pid_file = "/tmp/goglz.pid";
    
    if !std::path::Path::new(pid_file).exists() {
        println!("Goglz daemon is not running (no PID file found)");
        return Ok(());
    }

    let pid_content = std::fs::read_to_string(pid_file)?;
    let pid: u32 = pid_content.trim().parse()
        .map_err(|e| crate::error::GoglzError::ProcessingFailed(format!("Invalid PID: {}", e)))?;

    println!("Stopping goglz daemon (PID: {})...", pid);
    
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }

    // Wait a bit and then remove the PID file
    std::thread::sleep(std::time::Duration::from_secs(2));
    std::fs::remove_file(pid_file)?;
    
    println!("Goglz daemon stopped");
    Ok(())
}

fn show_status() -> Result<()> {
    let pid_file = "/tmp/goglz.pid";
    
    if !std::path::Path::new(pid_file).exists() {
        println!("Goglz daemon is not running");
        return Ok(());
    }

    let pid_content = std::fs::read_to_string(pid_file)?;
    let pid: u32 = pid_content.trim().parse()
        .map_err(|e| crate::error::GoglzError::ProcessingFailed(format!("Invalid PID: {}", e)))?;

    // Check if process is actually running
    unsafe {
        if libc::kill(pid as i32, 0) == 0 {
            println!("Goglz daemon is running (PID: {})", pid);
        } else {
            println!("Goglz daemon PID file exists but process is not running");
            println!("You may want to remove the stale PID file: {}", pid_file);
        }
    }

    Ok(())
}

fn init_config() -> Result<()> {
    let config_path = dirs::home_dir()
        .ok_or_else(|| crate::error::GoglzError::ProcessingFailed("Could not determine home directory".to_string()))?
        .join(".goglz");

    if config_path.exists() {
        println!("Configuration file already exists at {:?}", config_path);
        print!("Overwrite? [y/N]: ");
        std::io::stdout().flush()?;
        
        let stdin = std::io::stdin();
        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;
        
        if !input.trim().to_lowercase().starts_with('y') {
            println!("Aborted");
            return Ok(());
        }
    }

    let example_config = r#"# Goglz Configuration File

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
"#;

    std::fs::write(&config_path, example_config)?;
    println!("Example configuration written to {:?}", config_path);
    println!("Please edit this file to add your API keys and configure directories to monitor.");
    
    Ok(())
}
