//! Manager binary

use clap::Parser;
use distributed_downloader::config::ManagerConfig;
use distributed_downloader::network::ManagerNetwork;
use tracing::info;
use tracing_subscriber;

use std::net::TcpStream;

/// Distributed Downloader Manager
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the configuration file
    #[clap(short, long, default_value = "configs/manager.yml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<(), distributed_downloader::error::DistributedDownloaderError> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // Load configuration
    let config = ManagerConfig::from_file(&args.config)?;

    // Get manager IP address
    let manager_addr_ipv4 = get_local_ip()?;
    info!("Manager IP address: {}", manager_addr_ipv4);

    // Create manager network handler
    let manager = ManagerNetwork::new();

    info!("Starting manager on {}:{}", config.manager_addr_ipv4, config.manager_port);
    // Start listening
    manager.listen(&config.manager_addr_ipv4, config.manager_port).await?;

    Ok(())
}

/// Get local IP address
fn get_local_ip() -> Result<String, distributed_downloader::error::DistributedDownloaderError> {
    // Try to connect to a remote address to determine local IP
    let socket = TcpStream::connect("8.8.8.8:53")?;
    let local_addr = socket.local_addr()?;
    Ok(local_addr.ip().to_string())
}