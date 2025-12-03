//! Server binary for the Distributed Downloader.

use anyhow::Result;
use clap::Parser;
use futures_util::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Endpoint, Request};
use tracing::{error, info, warn};
use uuid::Uuid;

use distributed_downloader::{
    config::ServerConfig,
    downloader::http::download_range,
    proto::distributed_downloader::{
        manager_command::Payload as ManagerPayload,
        manager_service_client::ManagerServiceClient,
        server_message::Payload as ServerPayload, ChunkData, ManagerCommand, ServerMessage,
        ServerRegistration, TaskResult,
    },
};

type ServerMessageSender = mpsc::Sender<ServerMessage>;

/// Command-line arguments for the server.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the server configuration file.
    #[clap(short, long, default_value = "configs/server.yml")]
    config: String,
}

/// Main entry point for the server.
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    info!("Loading config from: {}", args.config);
    let config = ServerConfig::from_file(&args.config)?;
    let server_id = Uuid::new_v4().to_string();
    info!("Generated server ID: {}", server_id);

    loop {
        info!(
            "Attempting to connect to manager at: {}",
            format!("http://{}:{}", config.manager_addr_ipv4, config.manager_port)
        );
        match connect_and_listen(server_id.clone(), config.clone()).await {
            Ok(_) => warn!("Stream closed. Reconnecting in 5s."),
            Err(e) => error!("Connection failed: {}. Retrying in 5s.", e),
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

/// Connects to the manager, establishes a bidirectional stream, and listens for commands.
async fn connect_and_listen(server_id: String, config: ServerConfig) -> Result<()> {
    let address = format!(
        "http://{}:{}",
        config.manager_addr_ipv4, config.manager_port
    );

    let channel = Endpoint::new(address)?.connect().await?;
    let mut client = ManagerServiceClient::new(channel);
    info!("Successfully connected to manager.");

    let (tx, rx) = mpsc::channel(10); // Outbound channel
    let outbound_stream = ReceiverStream::new(rx);

    let registration = ServerRegistration {
        server_id: server_id.clone(),
        address: "self-reported-address:port".to_string(),
    };
    tx.send(ServerMessage {
        payload: Some(ServerPayload::Register(registration)),
    })
    .await?;
    info!("Sent registration message.");

    let response = client
        .establish_connection(Request::new(outbound_stream))
        .await?;
    let mut inbound_stream = response.into_inner();
    info!("Connection established. Listening for commands...");

    while let Some(result) = inbound_stream.next().await {
        match result {
            Ok(command) => {
                handle_manager_command(command, tx.clone()).await;
            }
            Err(status) => {
                error!("Error from manager: {}", status);
                break;
            }
        }
    }

    Ok(())
}

/// Handles a single command from the manager by spawning a worker task.
async fn handle_manager_command(command: ManagerCommand, tx: ServerMessageSender) {
    if let Some(ManagerPayload::AssignTask(task)) = command.payload {
        info!("Received download task: {}", task.task_id);
        tokio::spawn(async move {
            let task_for_result = task.clone(); // Clone for potential failure report
            let offset = task.range_start;

            let download_result = download_range(&task.url, task.range_start, task.range_end).await;

            let response_payload = match download_result {
                Ok(data) => {
                    info!("Task {} downloaded {} bytes successfully.", task.task_id, data.len());
                    ServerPayload::ChunkData(ChunkData {
                        job_id: task.job_id,
                        task_id: task.task_id,
                        offset,
                        data: data.into(),
                    })
                }
                Err(e) => {
                    error!("Task {} download failed: {}", task.task_id, e);
                    ServerPayload::TaskResult(TaskResult {
                        task: Some(task_for_result),
                        success: false,
                        error_message: e.to_string(),
                    })
                }
            };
            
            if tx.send(ServerMessage { payload: Some(response_payload) }).await.is_err() {
                warn!("Failed to send task result back to manager. Connection may be closed.");
            }
        });
    } else {
        warn!("Received an empty or unknown command.");
    }
}
