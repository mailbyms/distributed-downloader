//! Client binary

use clap::Parser;
use distributed_downloader::config::ClientConfig;
use distributed_downloader::network::{ClientNetwork};
use distributed_downloader::utils::{create_dir, remove_dir};
use tracing::info;
use tracing_subscriber;

use std::time::Instant;

/// Distributed Downloader Client
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// URL to download
    #[clap()]
    url: String,

    /// Path to the configuration file
    #[clap(short, long, default_value = "configs/client.yml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<(), distributed_downloader::error::DistributedDownloaderError> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // Load configuration
    let config = ClientConfig::from_file(&args.config)?;

    // Create working directories
    create_work_dirs(&config)?;

    // Extract filename from URL
    let filename = extract_filename(&args.url);
    let final_file_path = format!("{}{}", config.target_dir, filename);

    // Use the new download method
    info!("Sending download request to manager");

    // Start download timer
    let start_time = Instant::now();

    // Request download through manager
    ClientNetwork::request_download_via_manager(
        &config.manager_addr_ipv4,
        config.manager_port,
        &args.url,
        &final_file_path,
    ).await?;

    info!("File has been downloaded");

    // Calculate download speed
    let duration = start_time.elapsed();
    let file_size = std::fs::metadata(&final_file_path)?.len() as f64 / (1024.0 * 1024.0); // MB
    let speed = file_size / duration.as_secs_f64();

    info!("\n\nDownload speed = {:.2} MB/s", speed);
    info!("Time = {:.2} seconds", duration.as_secs_f64());

    // Clean up temporary directory
    remove_dir(&config.tmp_dir)?;

    Ok(())
}

/// Create working directories
fn create_work_dirs(config: &ClientConfig) -> Result<(), distributed_downloader::error::DistributedDownloaderError> {
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

/// Extract filename from URL
fn extract_filename(url: &str) -> String {
    let path = url.split('/').last().unwrap_or("downloaded_file");
    path.replace("%20", "_")
}

