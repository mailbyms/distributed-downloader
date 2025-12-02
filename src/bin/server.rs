//! Server binary

use clap::Parser;
use distributed_downloader::config::ServerConfig;
use distributed_downloader::network::ServerNetwork;
use distributed_downloader::utils::create_dir;
use tracing::{info, error};
use tracing_subscriber;
use tokio::time;

/// Distributed Downloader Server
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the configuration file
    #[clap(short, long, default_value = "configs/server.yml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<(), distributed_downloader::error::DistributedDownloaderError> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // Load configuration
    let config = ServerConfig::from_file(&args.config)?;

    // Create working directories
    create_work_dirs(&config)?;

    // Create server network handler
    let mut server = ServerNetwork::new(
        config.id,
        config.tmp_dir.clone(),
        config.target_dir.clone(),
        config.threads_num,
    );

    // Set manager and server info
    server.set_manager_info(config.manager_addr_ipv4.clone(), config.manager_port);

    // Attempt to establish connection with manager, with retry logic
    // The establish_long_connection function will also handle task listening
    loop {
        info!("Establishing long connection with manager...");

        match server.establish_long_connection().await {
            Ok(()) => {
                info!("Successfully connected to manager and listening for tasks");
                break; // Successfully connected and listening, exit the retry loop
            }
            Err(e) => {
                error!("Error establishing connection with manager: {}", e);
                info!("Retrying connection in 5 seconds...");
                time::sleep(time::Duration::from_secs(5)).await;
                // Continue the loop to retry connection
            }
        }
    }

    // Keep the server running and periodically check connection health
    // Note: In the current implementation, the server's main connection is used for listening to tasks,
    // so we can't use it for sending heartbeats without interfering with task listening.
    // A more advanced implementation would use a separate connection for heartbeats.
    loop {
        // Sleep for a while before checking connection health
        time::sleep(time::Duration::from_secs(30)).await;

        // In a more advanced implementation, we would check the connection health here
        // and attempt to reconnect if needed. For now, we'll just keep the server running.
        info!("Server is still running");
    }

}

/// Create working directories
fn create_work_dirs(config: &ServerConfig) -> Result<(), distributed_downloader::error::DistributedDownloaderError> {
    if !config.target_dir.is_empty() {
        create_dir(&config.target_dir)?;
    } else {
        eprintln!("Error: Please provide download path in config file.");
        std::process::exit(1);
    }

    if !config.tmp_dir.is_empty() {
        create_dir(&config.tmp_dir)?;
    } else {
        eprintln!("Error: Please provide temp path in config file.");
        std::process::exit(1);
    }

    Ok(())
}

