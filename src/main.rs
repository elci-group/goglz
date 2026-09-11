// SPDX-License-Identifier: MIT
use clap::{Parser, Subcommand};
use daemonize::Daemonize;
use goglz::ai_client::AiClient;
use goglz::config::{load_config, load_revise_config, MonitoredDirectory};
use goglz::error::{GoglzError, Result};
use goglz::monitor::DirectoryMonitor;
use goglz::planning::{self, PlanOptions};
use goglz::portfolio::{default_portfolio_patterns, discover_projects_default};
use goglz::processor::DocumentProcessor;
use goglz::revise::ReviseProcessor;
use std::fs::File;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing::info;

#[derive(Parser)]
#[command(name = "goglz")]
#[command(about = "A daemon that monitors directories and improves document clarity using AI", long_about = None)]
struct Cli {
    /// Apply the command across all goglz projects detected under the home directory
    #[arg(long, global = true)]
    portfolio: bool,

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
    /// Create, revise, or check a product strategy workspace
    Plan {
        #[command(subcommand)]
        command: PlanCommands,
    },
}

#[derive(Subcommand)]
enum PlanCommands {
    /// Generate the complete PRD, TRD, MVP, flow, brand, schema, and growth set
    Generate {
        #[arg(short, long)]
        directory: Option<PathBuf>,
        #[arg(long)]
        topic: Option<String>,
        #[arg(long)]
        force: bool,
    },
    /// Revise one artifact or the complete plan
    Revise {
        #[arg(short, long)]
        directory: Option<PathBuf>,
        #[arg(long)]
        artifact: Option<String>,
        #[arg(
            long,
            default_value = "Clarify assumptions, risks, and measurable next steps."
        )]
        instruction: String,
        /// Use the configured Groq model for a generative revision
        #[arg(long)]
        ai: bool,
    },
    /// Check completeness and cross-document consistency
    Check {
        #[arg(short, long)]
        directory: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start { foreground } => {
            if cli.portfolio {
                let overrides = build_portfolio_directories()?;
                start_daemon(foreground, Some(overrides)).await?;
            } else {
                start_daemon(foreground, None).await?;
            }
        }
        Commands::Stop => {
            warn_ignored_portfolio(cli.portfolio, "stop");
            stop_daemon()?;
        }
        Commands::Status => {
            warn_ignored_portfolio(cli.portfolio, "status");
            show_status()?;
        }
        Commands::InitConfig => {
            warn_ignored_portfolio(cli.portfolio, "init-config");
            init_config()?;
        }
        Commands::Revise { directory } => {
            if cli.portfolio {
                revise_portfolio().await?;
            } else {
                revise_documents(directory).await?;
            }
        }
        Commands::Plan { command } => match command {
            PlanCommands::Generate {
                directory,
                topic,
                force,
            } => {
                let root = directory.unwrap_or(std::env::current_dir()?);
                let written = planning::generate(&root, topic.as_deref(), force)?;
                println!(
                    "Generated {} planning artifact(s) in {}",
                    written,
                    root.display()
                );
            }
            PlanCommands::Revise {
                directory,
                artifact,
                instruction,
                ai,
            } => {
                let root = directory.unwrap_or(std::env::current_dir()?);
                let options = PlanOptions {
                    artifact,
                    instruction,
                    ai,
                };
                let revised =
                    planning::revise(&root, &options, || Ok(AiClient::new(&load_config()?)))
                        .await?;
                println!(
                    "Revised {} planning artifact(s) in {}",
                    revised,
                    root.display()
                );
            }
            PlanCommands::Check { directory } => {
                let root = directory.unwrap_or(std::env::current_dir()?);
                let report = planning::check(&root)?;
                for result in &report.results {
                    println!(
                        "{} {}",
                        if result.passed { "PASS" } else { "FAIL" },
                        result.name
                    );
                }
                if !report.passed {
                    return Err(GoglzError::ProcessingFailed("plan checks failed".into()));
                }
                println!("All planning checks passed");
            }
        },
    }

    Ok(())
}

fn home_directory() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| {
        GoglzError::ProcessingFailed("Could not determine home directory".to_string())
    })
}

fn build_portfolio_directories() -> Result<Vec<MonitoredDirectory>> {
    let home = home_directory()?;
    let projects = discover_projects_default(&home)?;

    if projects.is_empty() {
        println!("Warning: no goglz projects found under {:?}", home);
    } else {
        println!("Portfolio mode: discovered {} project(s)", projects.len());
        for project in &projects {
            println!("  - {:?}", project);
        }
    }

    let patterns = default_portfolio_patterns();
    Ok(projects
        .into_iter()
        .map(|path| MonitoredDirectory {
            path,
            file_patterns: patterns.clone(),
            recursive: true,
        })
        .collect())
}

fn warn_ignored_portfolio(portfolio: bool, command: &str) {
    if portfolio {
        println!(
            "Warning: --portfolio has no effect on the '{}' command",
            command
        );
    }
}

async fn revise_portfolio() -> Result<()> {
    let home = home_directory()?;
    let projects = discover_projects_default(&home)?;

    if projects.is_empty() {
        println!("Warning: no goglz projects found under {:?}", home);
        return Ok(());
    }

    println!("Portfolio mode: revising {} project(s)", projects.len());
    for project in &projects {
        println!("\n--- Project: {:?} ---", project);
        if let Err(e) = revise_documents(Some(project.clone())).await {
            eprintln!("Failed to revise {:?}: {}", project, e);
            // Continue with remaining projects.
        }
    }

    Ok(())
}

async fn start_daemon(
    foreground: bool,
    portfolio_directories: Option<Vec<MonitoredDirectory>>,
) -> Result<()> {
    // Load configuration
    let mut config = load_config()?;

    if let Some(directories) = portfolio_directories {
        config.directories = directories;
    }

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
    let pid: u32 = pid_content
        .trim()
        .parse()
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
    let pid: u32 = pid_content
        .trim()
        .parse()
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
        .ok_or_else(|| {
            GoglzError::ProcessingFailed("Could not determine home directory".to_string())
        })?
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

    let example_config = format!(
        r#"# Goglz Configuration File

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
api_key = "{}"
model_120b = "gpt-oss-120b"
model_20b = "gpt-oss-20b"

# Groq Configuration (for LLaMA inference)
[groq]
api_key = "{}"
model = "llama3-70b-8192"
api_endpoint = "https://api.groq.com/openai/v1"

# Processing settings
[processing]
output_directory = "~/.goglz_output"
max_file_size_mb = 10
batch_size = 5
debounce_interval_ms = 2000
"#,
        std::env::var("GPT_OSS_API_KEY").unwrap_or_default(),
        std::env::var("GROQ_API_KEY").unwrap_or_default()
    );

    std::fs::write(&config_path, example_config)?;
    println!("Example configuration written to {:?}", config_path);
    println!("Please edit this file to add your API keys and configure directories to monitor.");

    Ok(())
}

async fn revise_documents(directory: Option<PathBuf>) -> Result<()> {
    // Initialize tracing if it has not already been set up. In portfolio mode
    // this function is called once per project, so we must tolerate a repeat
    // initialization attempt.
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();

    // Determine project root (current directory or parent of target directory)
    let target_dir = match directory {
        Some(dir) => dir,
        None => std::env::current_dir()?,
    };
    let project_root = target_dir.clone();

    // Load main configuration for API keys
    let config = load_config()?;

    // Load revise configuration from goglz.yaml
    let revise_config = load_revise_config(&project_root)?;

    info!(
        "Loading revise configuration from project root: {:?}",
        project_root
    );
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
    println!(
        "  Successful: {}",
        results
            .iter()
            .filter(|r| matches!(r.status, goglz::processor::ProcessingStatus::Completed))
            .count()
    );
    println!(
        "  Failed: {}",
        results
            .iter()
            .filter(|r| !matches!(r.status, goglz::processor::ProcessingStatus::Completed))
            .count()
    );

    Ok(())
}
