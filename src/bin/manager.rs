//! Main binary for the Manager node.
//!
//! This binary starts the gRPC ManagerService, which listens for connections
//! from clients (to start downloads) and servers (to register for work).

use anyhow::Result;
use clap::Parser;
use tonic::transport::Server;
use tracing::info;

use distributed_downloader::{
    config::ManagerConfig,
    network::manager::ManagerServiceImpl,
    proto::distributed_downloader::manager_service_server::ManagerServiceServer,
};

/// Command-line arguments for the manager.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the manager configuration file.
    #[clap(short, long, default_value = "configs/manager.yml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging.
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    info!("Loading manager configuration from: {}", args.config);
    let config = ManagerConfig::from_file(&args.config)?;

    let addr = format!("{}:{}", config.manager_addr_ipv4, config.manager_port).parse()?;
    let manager_service = ManagerServiceImpl::new();
    let svc = ManagerServiceServer::new(manager_service)
        .max_decoding_message_size(16 * 1024 * 1024); // 16 MB

    info!("Manager gRPC service listening on {}", addr);

    // Build and start the tonic gRPC server.
    Server::builder()
        .add_service(svc)
        .serve(addr)
        .await?;

    Ok(())
}
