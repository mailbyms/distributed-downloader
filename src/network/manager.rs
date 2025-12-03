//! Implements the gRPC ManagerService for the Distributed Downloader.

use anyhow::Result;
use dashmap::DashMap;
use futures_util::Stream;
use std::{pin::Pin, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status, Streaming};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{
    proto::distributed_downloader::{
        client_data_chunk, manager_service_server::ManagerService as GrpcManagerService,
        server_message::Payload as ServerPayload, ClientDataChunk, DataChunk, DownloadRequest,
        FileMetadata, ManagerCommand, ServerMessage,
    },
    utils::distributor::Distributor,
};

// Type alias for the sender part of the channel to a server.
type ServerCommandSender = mpsc::Sender<Result<ManagerCommand, Status>>;
// Type alias for the sender part of the channel to a client.
type ClientDataSender = mpsc::Sender<Result<ClientDataChunk, Status>>;

/// Holds the command-sending channel for a connected server.
#[derive(Debug)]
pub struct ServerConnection {
    pub sender: ServerCommandSender,
}

/// Holds the data-sending channel for a connected client.
#[derive(Debug)]
pub struct ClientConnection {
    pub sender: ClientDataSender,
}

/// Manages server connections.
#[derive(Debug, Clone, Default)]
pub struct ConnectionManager {
    servers: Arc<DashMap<String, ServerConnection>>,
}

impl ConnectionManager {
    pub fn add_server(&self, server_id: String, sender: ServerCommandSender) {
        self.servers
            .insert(server_id, ServerConnection { sender });
    }
    pub fn remove_server(&self, server_id: &str) {
        self.servers.remove(server_id);
    }
    pub fn get_all_servers(&self) -> Vec<(String, ServerConnection)> {
        self.servers
            .iter()
            .map(|entry| {
                (
                    entry.key().clone(),
                    ServerConnection {
                        sender: entry.value().sender.clone(),
                    },
                )
            })
            .collect()
    }
}

/// The main struct for our gRPC Manager service.
#[derive(Debug)]
pub struct ManagerServiceImpl {
    servers: ConnectionManager,
    clients: Arc<DashMap<String, ClientConnection>>, // Key: job_id
    http_client: reqwest::Client,
}

impl ManagerServiceImpl {
    pub fn new() -> Self {
        Self {
            servers: ConnectionManager::default(),
            clients: Arc::new(DashMap::new()),
            http_client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
                .build()
                .expect("Failed to build HTTP client"),
        }
    }
}

// Implement the gRPC service trait for our ManagerServiceImpl.
#[tonic::async_trait]
impl GrpcManagerService for ManagerServiceImpl {
    type EstablishConnectionStream =
        Pin<Box<dyn Stream<Item = Result<ManagerCommand, Status>> + Send>>;
    type RequestDownloadStream =
        Pin<Box<dyn Stream<Item = Result<ClientDataChunk, Status>> + Send>>;

    /// Called by a server to establish a persistent bidirectional connection.
    async fn establish_connection(
        &self,
        request: Request<Streaming<ServerMessage>>,
    ) -> Result<Response<Self::EstablishConnectionStream>, Status> {
        let mut in_stream = request.into_inner();
        let (out_tx, out_rx) = mpsc::channel(128); // Channel for sending commands to the server.

        let server_connections = self.servers.clone();
        let client_connections = self.clients.clone();
        let server_id = Arc::new(Mutex::new(None));

        tokio::spawn(async move {
            // 1. Handle Registration
            if let Some(Ok(first_msg)) = in_stream.next().await {
                if let Some(ServerPayload::Register(reg)) = first_msg.payload {
                    let id = reg.server_id;
                    info!("Server registered with ID: {}", id);
                    *server_id.lock().await = Some(id.clone());
                    server_connections.add_server(id, out_tx);
                } else {
                    error!("First message was not registration. Closing.");
                    return;
                }
            } else {
                error!("Server disconnected before registering. Closing.");
                return;
            }

            // 2. Process subsequent messages
            while let Some(result) = in_stream.next().await {
                match result {
                    Ok(msg) => {
                        if let Some(payload) = msg.payload {
                            match payload {
                                ServerPayload::ChunkData(chunk) => {
                                    // Forward chunk data to the correct client
                                    if let Some(client) = client_connections.get(&chunk.job_id) {
                                        let client_chunk = ClientDataChunk {
                                            payload: Some(client_data_chunk::Payload::Chunk(DataChunk {
                                                job_id: chunk.job_id,
                                                offset: chunk.offset,
                                                data: chunk.data,
                                            })),
                                        };
                                        if client.sender.send(Ok(client_chunk)).await.is_err() {
                                            warn!("Client for job {} disconnected. Cannot forward chunk.", chunk.task_id);
                                        }
                                    } else {
                                        warn!("Received chunk for unknown job_id: {}. Discarding.", chunk.job_id);
                                    }
                                }
                                ServerPayload::TaskResult(res) => {
                                    info!("Server reports task '{}' finished. Success: {}", res.task_id, res.success);
                                }
                                ServerPayload::Register(_) => {
                                    warn!("Duplicate registration message. Ignoring.");
                                }
                            }
                        }
                    }
                    Err(status) => {
                        error!("Error from server stream: {}. Closing.", status);
                        break;
                    }
                }
            }

            // 3. Cleanup on disconnect
            if let Some(id) = &*server_id.lock().await {
                info!("Server {} disconnected.", id);
                server_connections.remove_server(id);
            }
        });

        let out_stream = tokio_stream::wrappers::ReceiverStream::new(out_rx);
        Ok(Response::new(Box::pin(out_stream)))
    }

    /// Called by a client to request a new file download.
    async fn request_download(
        &self,
        request: Request<DownloadRequest>,
    ) -> Result<Response<Self::RequestDownloadStream>, Status> {
        let req = request.into_inner();
        let job_id = Uuid::new_v4().to_string();
        info!("({}) Received download request for URL: {}", job_id, req.url);

        let (client_tx, client_rx) = mpsc::channel(128);

        // Store the sender so we can forward data to this client
        self.clients
            .insert(job_id.clone(), ClientConnection { sender: client_tx.clone() });

        let http_client = self.http_client.clone();
        let server_connections = self.servers.clone();
        let clients = self.clients.clone();
        let url = req.url;

        // Spawn a master task for this download job
        tokio::spawn(async move {
            let file_size: u64;
            
            // 1. Get file metadata
            match http_client.get(&url).send().await {
                Ok(res) => {
                    let final_url = res.url().clone();
                    file_size = match res
                        .headers()
                        .get(reqwest::header::CONTENT_LENGTH)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse().ok())
                    {
                        Some(size) => size,
                        None => {
                            let _ = client_tx.send(Err(Status::internal("Could not get content length."))).await;
                            clients.remove(&job_id);
                            return;
                        }
                    };
                    info!("({}) Final URL: {}. Size: {}", job_id, final_url, file_size);
                    
                    // 2. Send metadata to client
                    let metadata = FileMetadata { job_id: job_id.clone(), file_size };
                    if client_tx.send(Ok(ClientDataChunk { payload: Some(client_data_chunk::Payload::Metadata(metadata)) })).await.is_err() {
                        warn!("({}) Client disconnected before receiving metadata.", job_id);
                        clients.remove(&job_id);
                        return;
                    }

                    // 3. Distribute download tasks
                    let distributor = Distributor::new(server_connections);
                    if let Err(e) = distributor.distribute_and_dispatch(job_id.clone(), final_url.to_string(), file_size).await {
                        error!("({}) Failed to dispatch tasks: {}", job_id, e);
                        let _ = client_tx.send(Err(Status::internal("Failed to dispatch tasks."))).await;
                        clients.remove(&job_id);
                        return;
                    }
                }
                Err(e) => {
                    error!("({}) Failed to get file info: {}", job_id, e);
                    let _ = client_tx.send(Err(Status::internal(format!("Failed to get file info: {}", e)))).await;
                    clients.remove(&job_id);
                    return;
                }
            }
            
            // The spawned task for this job ends here.
            // The forwarding of chunks will happen in the `establish_connection` method.
            // A separate task could monitor the overall job progress and close the client stream,
            // or remove the client connection from the map when all chunks are received.
            // For now, we leave the stream open.
        });

        let out_stream = tokio_stream::wrappers::ReceiverStream::new(client_rx);
        Ok(Response::new(Box::pin(out_stream)))
    }
}