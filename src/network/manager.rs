//! Manager network communication

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::{Arc, Mutex};
use crate::error::Result;
use std::collections::HashMap;
use tokio::sync::Mutex as TokioMutex;
use crate::proto::distributed_downloader::*;
use prost::Message as ProstMessage;

/// Information about a server
pub type ServerInfo = crate::proto::distributed_downloader::ServerInfo;

/// Download request from client
pub type DownloadRequest = crate::proto::distributed_downloader::DownloadRequest;

/// File information response to client
pub type FileInfoResponse = crate::proto::distributed_downloader::FileInfoResponse;

/// Task to be sent to server
pub type DownloadTask = crate::proto::distributed_downloader::DownloadTask;

/// Information about a download task for tracking purposes
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub start_position: u64,
    pub end_position: u64,
    pub assigned_server: Option<ServerInfo>, // Track which server is assigned to this task
    pub completed: bool, // Track if this task is completed
}

/// Manager network handler
pub struct ManagerNetwork {
    server_list: Arc<Mutex<Vec<ServerInfo>>>,
    // Map of task_id to client socket for forwarding data
    pending_tasks: Arc<TokioMutex<HashMap<String, TcpStream>>>,
    // Map of server info to server connection for pushing tasks
    server_connections: Arc<TokioMutex<HashMap<ServerInfo, TcpStream>>>,
    // Map of task_id to task info for tracking download intervals
    task_infos: Arc<TokioMutex<HashMap<String, TaskInfo>>>,
}

impl ManagerNetwork {
    /// Create a new manager network handler
    pub fn new() -> Self {
        Self {
            server_list: Arc::new(Mutex::new(Vec::new())),
            pending_tasks: Arc::new(TokioMutex::new(HashMap::new())),
            server_connections: Arc::new(TokioMutex::new(HashMap::new())),
            task_infos: Arc::new(TokioMutex::new(HashMap::new())),
        }
    }

    /// Start listening for connections
    pub async fn listen(&self, addr: &str, port: u16) -> Result<()> {
        let listener = TcpListener::bind(format!("{}:{}", addr, port)).await?;
        println!("Manager listening on {}:{}", addr, port);

        loop {
            let (socket, peer_addr) = listener.accept().await?;
            println!("New connection from: {}", peer_addr);

            let server_list = self.server_list.clone();
            let pending_tasks = self.pending_tasks.clone();
            let server_connections = self.server_connections.clone();
            let task_infos = self.task_infos.clone();

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(socket, server_list, pending_tasks, server_connections, task_infos).await {
                    eprintln!("Error handling connection: {}", e);
                }
            });
        }
    }

    /// Handle an incoming connection
    async fn handle_connection(
        mut socket: TcpStream,
        server_list: Arc<Mutex<Vec<ServerInfo>>>,
        pending_tasks: Arc<TokioMutex<HashMap<String, TcpStream>>>,
        server_connections: Arc<TokioMutex<HashMap<ServerInfo, TcpStream>>>,
        task_infos: Arc<TokioMutex<HashMap<String, TaskInfo>>>,
    ) -> Result<()> {
        // Read initial message to determine connection type
        let mut buffer = [0; 2048];
        let n = socket.read(&mut buffer).await?;
        let message = String::from_utf8_lossy(&buffer[..n]);

        match message.as_ref() {
            "server_register" => {
                socket.write_all(b"go_ahead").await?;

                let mut buffer = [0; 2048];
                let n = socket.read(&mut buffer).await?;
                let message: Message = ProstMessage::decode(&buffer[..n])?;

                if let Some(message::Payload::ServerRegister(server_register)) = message.payload {
                    let server_info = ServerInfo { id: server_register.server_id };

                    // Store server info and connection
                    {
                        let mut list: std::sync::MutexGuard<'_, Vec<ServerInfo>> = server_list.lock().unwrap();
                        // Check if server already exists to avoid duplicates
                        if !list.iter().any(|s| s.id == server_info.id) {
                            list.push(server_info.clone());
                            println!("Added server: {}", server_info.id);
                        } else {
                            println!("Server already registered: {}", server_info.id);
                        }
                    }

                    // Send final acknowledgment
                    socket.write_all(b"registered").await?;

                    println!("Server list: {:?}", server_list.lock().unwrap());

                    // Maintain persistent connection by spawning a task to handle messages from this server
                    let server_info_clone = server_info.clone();
                    let server_connections_clone = server_connections.clone();

                    tokio::spawn(async move {
                        // Store the connection for future use
                        {
                            let mut connections = server_connections_clone.lock().await;
                            connections.insert(server_info_clone.clone(), socket);
                        }

                        // Connection is now maintained for future communication
                        println!("Maintaining persistent connection with server {}", server_info_clone.id);

                        // In a more advanced implementation, we could periodically check the connection health
                        // and remove stale connections. For now, we'll rely on the server to notify us when it goes down.
                    });
                }
            },
            "server_down" => {
                socket.write_all(b"go_ahead").await?;

                let mut buffer = [0; 2048];
                let n = socket.read(&mut buffer).await?;
                let message: Message = ProstMessage::decode(&buffer[..n])?;

                if let Some(message::Payload::ServerDown(server_down)) = message.payload {
                    let server_info = ServerInfo { id: server_down.server_id };

                    {
                        let mut list = server_list.lock().unwrap();
                        list.retain(|s| s.id != server_info.id);
                        println!("Removed server: {}", server_info.id);
                    }

                    // Also remove the connection from the server_connections map
                    {
                        let mut connections = server_connections.lock().await;
                        connections.remove(&server_info);
                        println!("Removed connection for server: {}", server_info.id);
                    }

                    println!("Server list: {:?}", server_list.lock().unwrap());
                }
            },
            "ask_for_server_list" => {
                let list = {
                    let list = server_list.lock().unwrap();
                    list.clone()
                };

                // Create a Message containing all server info
                for server_info in list {
                    let message = crate::proto::distributed_downloader::Message {
                        payload: Some(message::Payload::ServerInfo(server_info)),
                    };
                    let encoded = message.encode_to_vec();
                    socket.write_all(&encoded).await?;
                }
                println!("Sent server list to client");
            },
            "download_request" => {
                // Client is sending a download request
                socket.write_all(b"go_ahead").await?;

                let mut buffer = [0; 4096];
                let n = socket.read(&mut buffer).await?;
                let message: Message = ProstMessage::decode(&buffer[..n])?;

                if let Some(message::Payload::DownloadRequest(download_request)) = message.payload {
                    println!("Received download request for URL: {}", download_request.url);

                    // Get file information and send it to client
                    if let Ok(file_info) = Self::get_file_info(&download_request.url).await {
                        // Send file info to client
                        let file_info_msg = Message {
                            payload: Some(message::Payload::FileInfoResponse(file_info.clone())),
                        };
                        let encoded = file_info_msg.encode_to_vec();
                        socket.write_all(&encoded).await?;

                        // Wait for client acknowledgment
                        let mut ack_buffer = [0; 2048];
                        let ack_n = socket.read(&mut ack_buffer).await?;
                        let ack_message = String::from_utf8_lossy(&ack_buffer[..ack_n]);

                        if ack_message == "ready_for_download" {
                            // Distribute the download task to available servers
                            Self::distribute_download_task(
                                download_request,
                                server_list,
                                pending_tasks,
                                task_infos,
                                socket,
                                server_connections,
                                file_info,
                            ).await?;
                        }
                    } else {
                        eprintln!("Failed to get file information for URL: {}", download_request.url);
                    }
                }
            },
            _ => {
                // Try to decode as a protobuf message
                if let Ok(message) = ProstMessage::decode(buffer[..n].as_ref()) {
                    let message: crate::proto::distributed_downloader::Message = message;
                    match message.payload {
                        Some(message::Payload::TaskResult(task_result)) => {
                            println!("Received task result for task: {}", task_result.task_id);

                            // Mark the task as completed
                            let mut infos = task_infos.lock().await;
                            if let Some(task_info) = infos.get_mut(&task_result.task_id) {
                                task_info.completed = true;
                                println!("Marked task {} as completed", task_result.task_id);
                            } else {
                                eprintln!("Task info not found for task: {}", task_result.task_id);
                            }
                            drop(infos); // Release the lock

                            // Assign new pending tasks to available servers
                            // We need to get the final_url - for now we'll use a placeholder
                            // In a real implementation, we would store the URL with the task or retrieve it from somewhere
                            Self::assign_pending_tasks(server_list.clone(), task_infos.clone(), server_connections.clone(), "".to_string()).await?;

                            // Forward the data to the waiting client
                            // First, get the task info to get the download interval
                            let task_info = {
                                let infos = task_infos.lock().await;
                                infos.get(&task_result.task_id).cloned() // Get the task info without removing it
                            };

                            if let Some(task_info) = task_info {
                                let mut tasks = pending_tasks.lock().await;
                                // Use the client_id we stored earlier instead of task_id
                                // For now, we'll use a placeholder - in a real implementation we'd track this properly
                                if let Some(client_socket) = tasks.values_mut().next() {
                                    // Send the task ID and download interval
                                    let interval_msg = format!("task_data:{}:{}-{}", task_result.task_id, task_info.start_position, task_info.end_position);
                                    client_socket.write_all(interval_msg.as_bytes()).await?;

                                    // Then forward the rest of the data
                                    let remaining_data = &buffer[n..];
                                    if !remaining_data.is_empty() {
                                        client_socket.write_all(remaining_data).await?;
                                    }

                                    // Continue reading and forwarding data
                                    loop {
                                        let n = match socket.read(&mut buffer).await {
                                            Ok(0) => break, // Connection closed
                                            Ok(n) => n,
                                            Err(_) => break, // Error occurred
                                        };

                                        if let Err(_) = client_socket.write_all(&buffer[..n]).await {
                                            break; // Client disconnected
                                        }
                                    }
                                }
                            } else {
                                eprintln!("Task info not found for task: {}", task_result.task_id);
                            }
                        }
                        Some(message::Payload::FileData(file_data)) => {
                            // Handle file data
                            println!("Received file data: {} bytes", file_data.len());

                            // Forward the data to the waiting client
                            let mut tasks = pending_tasks.lock().await;
                            if let Some(client_socket) = tasks.values_mut().next() {
                                client_socket.write_all(&file_data).await?;
                            }
                        }
                        _ => {
                            eprintln!("Unknown protobuf message payload");
                        }
                    }
                } else {
                    eprintln!("Unknown message: {}", message);
                }
            }
        }

        Ok(())
    }

    /// Send a download task to a server through a long-lived connection
    async fn send_task_to_server_through_connection(
        server_info: &ServerInfo,
        task: DownloadTask,
        server_connections: Arc<TokioMutex<HashMap<ServerInfo, TcpStream>>>,
    ) -> Result<()> {
        // Get the connection for this server
        let mut connections = server_connections.lock().await;
        if let Some(socket) = connections.get_mut(server_info) {
            // Create protobuf message for the task
            let message = crate::proto::distributed_downloader::Message {
                payload: Some(message::Payload::DownloadTask(task)),
            };
            let encoded = message.encode_to_vec();

            // Send the task details directly (no need for separate command and ack)
            if let Err(e) = socket.write_all(&encoded).await {
                eprintln!("Failed to send task details to server {}: {}", server_info.id, e);
                // Remove the broken connection
                connections.remove(server_info);
                // Fall back to creating a new connection
                return Self::send_task_to_server(server_info, message).await;
            }
            Ok(())
        } else {
            // If we don't have a connection, fall back to creating a new one
            let message = crate::proto::distributed_downloader::Message {
                payload: Some(message::Payload::DownloadTask(task)),
            };
            Self::send_task_to_server(server_info, message).await
        }
    }

    /// Get file information from URL
    async fn get_file_info(url: &str) -> Result<FileInfoResponse> {
        // Create a client that follows redirects with a browser-like User-Agent
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10)) // Allow up to 10 redirects
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .build()?;

        // First, resolve any redirects by sending a GET request (but not downloading the body)
        let resolved_url_response = client
            .get(url)
            .send()
            .await?;

        // Get the final URL after redirects
        let final_url = resolved_url_response.url().clone();
        print!("Final URL after redirects: {}\n", final_url);

        // Now send a HEAD request to the final URL to get file size
        let head_response = client
            .head(final_url.as_str())
            .send()
            .await?;

        // 打印出响应头以供调试
        // println!("HEAD response headers: {:#?}", head_response.headers());

        // If HEAD request fails, try to get file size from GET request
        let file_size = if head_response.status().is_success() {
            head_response.headers().get("Content-Length")
                .and_then(|val| val.to_str().ok())
                .and_then(|val| val.parse::<u64>().ok())
                .unwrap_or(1000000) // Default to 1MB if we can't get the file size
        } else {
            // As a fallback, try to get file size from the GET response we already made
            resolved_url_response.headers().get("Content-Length")
                .and_then(|val| val.to_str().ok())
                .and_then(|val| val.parse::<u64>().ok())
                .unwrap_or(1000000) // Default to 1MB if we can't get the file size
        };

        println!("File size: {} bytes", file_size);

        // Calculate chunk sizes with maximum 10MB per chunk
        const MAX_CHUNK_SIZE: u64 = 10 * 1024 * 1024; // 10MB in bytes
        let chunk_count = ((file_size + MAX_CHUNK_SIZE - 1) / MAX_CHUNK_SIZE) as usize; // Ceiling division
        let chunk_size = MAX_CHUNK_SIZE;
        let mut chunk_sizes = vec![chunk_size; chunk_count];

        // Adjust the last chunk size to match the actual file size
        if chunk_count > 0 {
            let last_chunk_index = chunk_count - 1;
            let actual_last_chunk_size = file_size - (last_chunk_index as u64 * chunk_size);
            chunk_sizes[last_chunk_index] = actual_last_chunk_size;
        }

        Ok(FileInfoResponse {
            file_size,
            chunk_count: chunk_count as u32,
            chunk_sizes,
            final_url: final_url.to_string(),
        })
    }

    /// Distribute download task to servers
    async fn distribute_download_task(
        _download_request: DownloadRequest,
        server_list: Arc<Mutex<Vec<ServerInfo>>>,
        pending_tasks: Arc<TokioMutex<HashMap<String, TcpStream>>>,
        task_infos: Arc<TokioMutex<HashMap<String, TaskInfo>>>,
        client_socket: TcpStream,
        server_connections: Arc<TokioMutex<HashMap<ServerInfo, TcpStream>>>,
        file_info: FileInfoResponse,
    ) -> Result<()> {
        let servers = {
            let list = server_list.lock().unwrap();
            list.clone()
        };

        if servers.is_empty() {
            eprintln!("No servers available for download");
            return Ok(());
        }

        // Split the file range based on chunk sizes
        let mut intervals = Vec::new();
        let mut start = 0u64;
        for &chunk_size in &file_info.chunk_sizes {
            let end = start + chunk_size - 1;
            intervals.push((start, end));
            start = end + 1;
        }

        // Create task info for all chunks but don't assign them yet
        // We'll assign tasks dynamically as servers become available
        for (i, (start_pos, end_pos)) in intervals.iter().enumerate() {
            let task_id = format!("task_{}", i);

            // Create task info
            let task_info = TaskInfo {
                start_position: *start_pos,
                end_position: *end_pos,
                assigned_server: None, // Not assigned yet
                completed: false,
            };

            let mut infos = task_infos.lock().await;
            infos.insert(task_id, task_info);
        }

        // Store the client socket
        let client_id = uuid::Uuid::new_v4().to_string();
        let mut tasks = pending_tasks.lock().await;
        tasks.insert(client_id.clone(), client_socket);

        // Assign initial tasks to available servers
        Self::assign_pending_tasks(server_list, task_infos.clone(), server_connections.clone(), file_info.final_url.clone()).await?;

        Ok(())
    }

    /// Assign pending tasks to available servers
    async fn assign_pending_tasks(
        server_list: Arc<Mutex<Vec<ServerInfo>>>,
        task_infos: Arc<TokioMutex<HashMap<String, TaskInfo>>>,
        server_connections: Arc<TokioMutex<HashMap<ServerInfo, TcpStream>>>,
        final_url: String,
    ) -> Result<()> {
        let servers = {
            let list = server_list.lock().unwrap();
            list.clone()
        };

        if servers.is_empty() {
            return Ok(());
        }

        // Find unassigned tasks and assign them to servers
        let task_ids_to_assign: Vec<String> = {
            let infos = task_infos.lock().await;
            infos.iter()
                .filter(|(_, task_info)| task_info.assigned_server.is_none() && !task_info.completed)
                .map(|(task_id, _)| task_id.clone())
                .collect()
        };

        let mut server_index = 0;
        for task_id in task_ids_to_assign {
            let server = &servers[server_index % servers.len()];

            // Update task info
            {
                let mut infos = task_infos.lock().await;
                if let Some(task_info) = infos.get_mut(&task_id) {
                    task_info.assigned_server = Some(server.clone());
                }
            }

            // Get task info for creating download task
            let task_info = {
                let infos = task_infos.lock().await;
                infos.get(&task_id).cloned()
            };

            if let Some(task_info) = task_info {
                // Create download task
                let download_task = DownloadTask {
                    url: final_url.clone(),
                    start_position: task_info.start_position,
                    end_position: task_info.end_position,
                    task_id: task_id.clone(),
                };

                // Send task to server through long-lived connection
                if let Err(e) = Self::send_task_to_server_through_connection(server, download_task, server_connections.clone()).await {
                    eprintln!("Failed to send task to server: {}", e);
                    // Mark task as unassigned so it can be reassigned
                    let mut infos = task_infos.lock().await;
                    if let Some(task_info) = infos.get_mut(&task_id) {
                        task_info.assigned_server = None;
                    }
                } else {
                    println!("Sent download task {} to server:{} with interval [{}, {}]",
                             task_id, server.id, task_info.start_position, task_info.end_position);
                }

                server_index += 1;
            }
        }

        Ok(())
    }

    /// Send a download task to a server
    async fn send_task_to_server(server_info: &ServerInfo, message: Message) -> Result<()> {
        let mut socket = TcpStream::connect(format!("{}:{}", "127.0.0.1", server_info.id)).await?;

        // Send the task details directly as protobuf message
        let encoded = message.encode_to_vec();
        socket.write_all(&encoded).await?;

        Ok(())
    }

    /// Get the current server list
    pub fn get_server_list(&self) -> Vec<ServerInfo> {
        let list = self.server_list.lock().unwrap();
        list.clone()
    }
}