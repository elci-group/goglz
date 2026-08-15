use goglz::ai_client::AiClient;
use goglz::config::{load_config, load_revise_config};
use goglz::error::{GoglzError, Result};
use goglz::monitor::DirectoryMonitor;
use goglz::processor::DocumentProcessor;
use goglz::revise::ReviseProcessor;
use clap::{Parser, Subcommand};
use daemonize::Daemonize;
use std::fs::File;
use std::io::{Write, BufRead};
use std::path::PathBuf;
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
    /// Revise documents in a directory according to goglz.yaml
    Revise {
        /// Directory to revise (default: current working directory)
        #[arg(short, long)]
        directory: Option<PathBuf>,
    },
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
        Commands::Revise { directory } => {
            revise_documents(directory).await?;
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
                return Err(GoglzError::Daemonization(e.to_string()));
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
        .map_err(|e| GoglzError::ProcessingFailed(format!("Invalid PID: {}", e)))?;

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
        .map_err(|e| GoglzError::ProcessingFailed(format!("Invalid PID: {}", e)))?;

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
        .ok_or_else(|| GoglzError::ProcessingFailed("Could not determine home directory".to_string()))?
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

async fn revise_documents(directory: Option<PathBuf>) -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Determine project root (current directory or parent of target directory)
    let target_dir = directory.unwrap_or_else(|| std::env::current_dir().unwrap());
    let project_root = target_dir.clone();

    // Load main configuration for API keys
    let config = load_config()?;

    // Load revise configuration from goglz.yaml
    let revise_config = load_revise_config(&project_root)?;

    info!("Loading revise configuration from project root: {:?}", project_root);
    info!("Target directory: {:?}", target_dir);

    // Initialize AI client
    let ai_client = AiClient::new(&config);

    // Create revise processor
    let processor = ReviseProcessor::new(ai_client, revise_config, project_root, Some(target_dir));

    // Run revision process
    let results = processor.run().await?;

    // Print summary
    println!("\nRevision Summary:");
    println!("  Total documents processed: {}", results.len());
    println!("  Successful: {}", results.iter().filter(|r| matches!(r.status, goglz::processor::ProcessingStatus::Completed)).count());
    println!("  Failed: {}", results.iter().filter(|r| !matches!(r.status, goglz::processor::ProcessingStatus::Completed)).count());

    Ok(())
}
