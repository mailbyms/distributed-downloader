//! Implements the gRPC ManagerService and the core state management logic.

use anyhow::Result;
use dashmap::DashMap;
use futures_util::Stream;
use std::{collections::VecDeque, pin::Pin, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status, Streaming};
use tracing::{info, warn};
use uuid::Uuid;

use crate::proto::distributed_downloader::{
    client_data_chunk, manager_service_server::ManagerService as GrpcManagerService,
    server_message::Payload as ServerPayload, ClientDataChunk, DataChunk, DownloadRequest,
    DownloadTask, FileMetadata, ManagerCommand, ServerMessage,
};

const MAX_CHUNK_SIZE: u64 = 3 * 1024 * 1024; // 3 MB

// ============== Type Definitions for State Management ==============

type ServerCommandSender = mpsc::Sender<Result<ManagerCommand, Status>>;
type ClientDataSender = mpsc::Sender<Result<ClientDataChunk, Status>>;
pub type TaskQueue = Arc<Mutex<VecDeque<DownloadTask>>>;

// ============== State Management Structs ==============

#[derive(Debug, Clone, PartialEq)]
pub enum ServerStatus {
    Idle,
    Busy(DownloadTask), // Holds the full task
}

/// Holds the state for a connected server.
#[derive(Debug)]
pub struct ServerConnection {
    pub sender: ServerCommandSender,
    pub status: Mutex<ServerStatus>,
}

/// Holds the state for an active download job.
#[derive(Debug)]
pub struct JobState {
    pub sender: ClientDataSender,
    pub total_chunks: u64,
    pub completed_chunks: u64,
}

/// Manages all connected servers using a thread-safe DashMap.
#[derive(Debug, Clone, Default)]
pub struct ServerManager {
    pub servers: Arc<DashMap<String, Arc<ServerConnection>>>,
}

/// Manages all active jobs and their clients.
#[derive(Debug, Clone, Default)]
pub struct JobManager {
    pub jobs: Arc<DashMap<String, Mutex<JobState>>>,
}

// ============== gRPC Service Implementation ==============

/// The main struct for our gRPC Manager service.
#[derive(Debug, Clone)]
pub struct ManagerServiceImpl {
    pub servers: ServerManager,
    pub jobs: JobManager,
    pub task_queue: TaskQueue,
    http_client: reqwest::Client,
}

impl ManagerServiceImpl {
    pub fn new() -> Self {
        Self {
            servers: ServerManager::default(),
            jobs: JobManager::default(),
            task_queue: Arc::new(Mutex::new(VecDeque::new())),
            http_client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
                .build()
                .expect("Failed to build HTTP client"),
        }
    }
}

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
        let (out_tx, out_rx) = mpsc::channel(128);

        let server_manager = self.servers.clone();
        let job_manager = self.jobs.clone();
        let task_queue = self.task_queue.clone();
        let server_id_arc = Arc::new(Mutex::new(None::<String>));

        tokio::spawn(async move {
            // 1. Handle Registration
            if let Some(Ok(first_msg)) = in_stream.next().await {
                if let Some(ServerPayload::Register(reg)) = first_msg.payload {
                    let id = reg.server_id;
                    info!("[Server: {}] Registered.", id);
                    let server_conn = Arc::new(ServerConnection {
                        sender: out_tx,
                        status: Mutex::new(ServerStatus::Idle),
                    });
                    *server_id_arc.lock().await = Some(id.clone());
                    server_manager.servers.insert(id, server_conn);
                } else { return; }
            } else { return; }

            // 2. Process subsequent messages
            while let Some(result) = in_stream.next().await {
                if let Ok(msg) = result {
                    if let Some(payload) = msg.payload {
                        
                        let mut job_to_remove = None;

                        match payload {
                            ServerPayload::ChunkData(chunk) => {
                                let mut job_is_complete = false;
                                // Find the job and client associated with this chunk
                                if let Some(job_entry) = job_manager.jobs.get(&chunk.job_id) {
                                    let mut job = job_entry.lock().await;
                                    let client_chunk = ClientDataChunk {
                                        payload: Some(client_data_chunk::Payload::Chunk(DataChunk {
                                            job_id: chunk.job_id.clone(),
                                            offset: chunk.offset,
                                            data: chunk.data,
                                        })),
                                    };
                                    // Forward chunk to client
                                    if job.sender.send(Ok(client_chunk)).await.is_err() {
                                        warn!("[Job: {}] Client disconnected. Flagging job for removal.", chunk.job_id);
                                        job_is_complete = true;
                                    } else {
                                        job.completed_chunks += 1;
                                        if job.completed_chunks >= job.total_chunks {
                                            job_is_complete = true;
                                        }
                                    }
                                } else {
                                    warn!("[Job: {}] Received chunk for a cancelled or unknown job. Discarding.", chunk.job_id);
                                }

                                if job_is_complete {
                                    job_to_remove = Some(chunk.job_id.clone());
                                }
                            }
                            ServerPayload::TaskResult(res) => {
                                if !res.success {
                                    if let Some(task) = res.task {
                                        warn!("[Manager] Re-queueing failed task {}.", task.task_id);
                                        task_queue.lock().await.push_front(task);
                                    }
                                }
                            }
                            _ => {}
                        }

                        // Set server status back to Idle after it has reported in
                        if let Some(id_str) = server_id_arc.lock().await.as_ref() {
                            if let Some(server_conn) = server_manager.servers.get(id_str) {
                                *server_conn.status.lock().await = ServerStatus::Idle;
                                info!("[Server: {}] Status set to Idle.", id_str);
                            }
                        }

                        // Perform job removal outside of any other locks
                        if let Some(id) = job_to_remove {
                             if let Some((_, job)) = job_manager.jobs.remove(&id) {
                                info!("[Job: {}] All chunks complete or client disconnected. Closing stream.", id);
                                drop(job); // This drops the sender, closing the stream
                             }
                        }
                    }
                }
            }

            // 3. Cleanup on server disconnect
            if let Some(id) = &*server_id_arc.lock().await {
                info!("[Server: {}] Disconnected.", id);
                if let Some((_server_id, server_conn)) = server_manager.servers.remove(id) {
                    let status = server_conn.status.lock().await;
                    if let ServerStatus::Busy(task) = &*status {
                        warn!("[Manager] Server {} disconnected while busy with task {}. Re-queueing.", id, task.task_id);
                        task_queue.lock().await.push_front(task.clone());
                    }
                }
            }
        });

        Ok(Response::new(Box::pin(tokio_stream::wrappers::ReceiverStream::new(out_rx))))
    }

    /// Called by a client to request a new file download.
    async fn request_download(
        &self,
        request: Request<DownloadRequest>,
    ) -> Result<Response<Self::RequestDownloadStream>, Status> {
        let req = request.into_inner();
        let job_id = Uuid::new_v4().to_string();
        info!("[Job: {}] Received download request for URL: {}", job_id, req.url);

        let (client_tx, client_rx) = mpsc::channel(128);
        
        let http_client = self.http_client.clone();
        let job_manager = self.jobs.clone();
        let task_queue = self.task_queue.clone();
        let url = req.url;

        tokio::spawn(async move {
            // 1. Get file metadata
            let response = match http_client.get(&url).send().await {
                Ok(res) => res,
                Err(_) => { /* ... error handling ... */ return; }
            };
            let final_url = response.url().clone();
            let file_size = match response.headers().get(reqwest::header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|s| s.parse().ok()) {
                Some(size) => size,
                None => { /* ... error handling ... */ return; }
            };
            info!("[Job: {}] Final URL: {}. Size: {}", job_id, final_url, file_size);

            // 2. Generate task chunks
            let mut tasks = VecDeque::new();
            if file_size > 0 {
                let mut current_pos = 0;
                while current_pos < file_size {
                    let end_pos = (current_pos + MAX_CHUNK_SIZE - 1).min(file_size - 1);
                    tasks.push_back(DownloadTask {
                        job_id: job_id.clone(),
                        task_id: Uuid::new_v4().to_string(),
                        url: final_url.to_string(),
                        range_start: current_pos,
                        range_end: end_pos,
                    });
                    current_pos = end_pos + 1;
                }
            }

            // 3. Register the job and enqueue tasks
            let total_chunks = tasks.len() as u64;
            let job_state = JobState {
                sender: client_tx.clone(),
                total_chunks,
                completed_chunks: 0,
            };
            job_manager.jobs.insert(job_id.clone(), Mutex::new(job_state));
            task_queue.lock().await.append(&mut tasks);
            
            // 4. Send metadata to client
            let metadata = FileMetadata { job_id: job_id.clone(), file_size };
            if client_tx.send(Ok(ClientDataChunk { payload: Some(client_data_chunk::Payload::Metadata(metadata)) })).await.is_err() {
                warn!("[Job: {}] Client disconnected before receiving metadata. Cleaning up.", job_id);
                job_manager.jobs.remove(&job_id);
                return;
            }
            
            // Handle zero-size files
            if file_size == 0 {
                info!("[Job: {}] Zero-size file. Closing client stream.", job_id);
                job_manager.jobs.remove(&job_id);
            }
        });

        Ok(Response::new(Box::pin(tokio_stream::wrappers::ReceiverStream::new(client_rx))))
    }
}