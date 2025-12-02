//! Manager network communication

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use crate::error::Result;
use std::collections::HashMap;
use tokio::sync::Mutex as TokioMutex;

/// Information about a server
#[derive(Debug, Clone, Serialize, Deserialize, Eq, Hash, PartialEq)]
pub struct ServerInfo {
    pub id: u16
}

/// Download request from client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub url: String,
}

/// File information response to client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfoResponse {
    pub file_size: u64,
    pub chunk_count: usize,
    pub chunk_sizes: Vec<u64>,
    pub final_url: String,
}

/// Task to be sent to server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub url: String,
    pub download_interval: [u64; 2],
    pub task_id: String,  // Unique identifier for this task
}

/// Manager network handler
pub struct ManagerNetwork {
    server_list: Arc<Mutex<Vec<ServerInfo>>>,
    // Map of task_id to client socket for forwarding data
    pending_tasks: Arc<TokioMutex<HashMap<String, TcpStream>>>,
    // Map of server info to server connection for pushing tasks
    server_connections: Arc<TokioMutex<HashMap<ServerInfo, TcpStream>>>,
}

impl ManagerNetwork {
    /// Create a new manager network handler
    pub fn new() -> Self {
        Self {
            server_list: Arc::new(Mutex::new(Vec::new())),
            pending_tasks: Arc::new(TokioMutex::new(HashMap::new())),
            server_connections: Arc::new(TokioMutex::new(HashMap::new())),
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

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(socket, server_list, pending_tasks, server_connections).await {
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
                let server_info_str = String::from_utf8_lossy(&buffer[..n]);
                let server_info: ServerInfo = serde_json::from_str(&server_info_str)?;

                // Store server info and connection
                {
                    let mut list: std::sync::MutexGuard<'_, Vec<ServerInfo>> = server_list.lock().unwrap();
                    // Check if server already exists to avoid duplicates
                    if !list.contains(&server_info) {
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
            },
            "server_down" => {
                socket.write_all(b"go_ahead").await?;

                let mut buffer = [0; 2048];
                let n = socket.read(&mut buffer).await?;
                let server_info_str = String::from_utf8_lossy(&buffer[..n]);
                let server_info: ServerInfo = serde_json::from_str(&server_info_str)?;

                {
                    let mut list = server_list.lock().unwrap();
                    list.retain(|s| !(s.id == server_info.id));
                    println!("Removed server: {}", server_info.id);
                }

                // Also remove the connection from the server_connections map
                {
                    let mut connections = server_connections.lock().await;
                    connections.remove(&server_info);
                    println!("Removed connection for server: {}", server_info.id);
                }

                println!("Server list: {:?}", server_list.lock().unwrap());
            },
            "ask_for_server_list" => {
                let server_list_json = {
                    let list = server_list.lock().unwrap();
                    serde_json::to_string(&*list)?
                };
                socket.write_all(server_list_json.as_bytes()).await?;
                println!("Sent server list to client");
            },
            "download_request" => {
                // Client is sending a download request
                socket.write_all(b"go_ahead").await?;

                let mut buffer = [0; 4096];
                let n = socket.read(&mut buffer).await?;
                let request_str = String::from_utf8_lossy(&buffer[..n]);
                let download_request: DownloadRequest = serde_json::from_str(&request_str)?;

                println!("Received download request for URL: {}", download_request.url);

                // Get file information and send it to client
                if let Ok(file_info) = Self::get_file_info(&download_request.url).await {
                    // Send file info to client
                    let file_info_str = serde_json::to_string(&file_info)?;
                    let response = format!("file_info:{}", file_info_str);
                    socket.write_all(response.as_bytes()).await?;

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
                            socket,
                            server_connections,
                            file_info,
                        ).await?;
                    }
                } else {
                    eprintln!("Failed to get file information for URL: {}", download_request.url);
                }
            },
            _ => {
                // Check if this is a server response with file data
                if message.starts_with("task_complete:") {
                    // This is a server sending completed task data
                    let parts: Vec<&str> = message.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        let task_id = parts[1].to_string();
                        println!("Received completed task data for task: {}", task_id);

                        // Forward the data to the waiting client
                        let mut tasks = pending_tasks.lock().await;
                        if let Some(mut client_socket) = tasks.remove(&task_id) {
                            // Send the task ID first
                            client_socket.write_all(format!("task_data:{}", task_id).as_bytes()).await?;

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
            // Send task command
            if let Err(e) = socket.write_all(b"download_task").await {
                eprintln!("Failed to send task command to server {}: {}", server_info.id, e);
                // Remove the broken connection
                connections.remove(server_info);
                // Fall back to creating a new connection
                return Self::send_task_to_server(server_info, task).await;
            }

            // Wait for acknowledgment
            let mut buffer = [0; 2048];
            let n = match socket.read(&mut buffer).await {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Failed to read acknowledgment from server {}: {}", server_info.id, e);
                    // Remove the broken connection
                    connections.remove(server_info);
                    // Fall back to creating a new connection
                    return Self::send_task_to_server(server_info, task).await;
                }
            };

            let response = String::from_utf8_lossy(&buffer[..n]);

            if response == "go_ahead" {
                // Send the task details
                let task_str = serde_json::to_string(&task)?;
                if let Err(e) = socket.write_all(task_str.as_bytes()).await {
                    eprintln!("Failed to send task details to server {}: {}", server_info.id, e);
                    // Remove the broken connection
                    connections.remove(server_info);
                    // Fall back to creating a new connection
                    return Self::send_task_to_server(server_info, task).await;
                }
                Ok(())
            } else {
                eprintln!("Server {} did not acknowledge task: {}", server_info.id, response);
                Err(crate::error::DistributedDownloaderError::NetworkError("Server did not acknowledge task".to_string()))
            }
        } else {
            // If we don't have a connection, fall back to creating a new one
            Self::send_task_to_server(server_info, task).await
        }
    }

    /// Get file information from URL
    async fn get_file_info(url: &str) -> Result<FileInfoResponse> {
        // Create a client that follows redirects
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10)) // Allow up to 10 redirects
            .build()?;

        // First, resolve any redirects by sending a GET request (but not downloading the body)
        let resolved_url_response = client
            .get(url)
            .send()
            .await?;

        // Get the final URL after redirects
        let final_url = resolved_url_response.url().clone();

        // Now send a HEAD request to the final URL to get file size
        let head_response = client
            .head(final_url.as_str())
            .send()
            .await?;

        let file_size = head_response.headers().get("content-length")
            .and_then(|val| val.to_str().ok())
            .and_then(|val| val.parse::<u64>().ok())
            .unwrap_or(1000000); // Default to 1MB if we can't get the file size

        println!("File size: {} bytes", file_size);

        // For simplicity, we'll assume a fixed chunk count for now
        // In a real implementation, this would be based on available servers
        let chunk_count = 4;
        let chunk_size = file_size / chunk_count as u64;
        let mut chunk_sizes = vec![chunk_size; chunk_count];

        // Adjust the last chunk size to account for any remainder
        if file_size % chunk_count as u64 != 0 {
            let remainder = file_size % chunk_count as u64;
            *chunk_sizes.last_mut().unwrap() += remainder;
        }

        Ok(FileInfoResponse {
            file_size,
            chunk_count,
            chunk_sizes,
            final_url: final_url.to_string(),
        })
    }

    /// Distribute download task to servers
    async fn distribute_download_task(
        download_request: DownloadRequest,
        server_list: Arc<Mutex<Vec<ServerInfo>>>,
        pending_tasks: Arc<TokioMutex<HashMap<String, TcpStream>>>,
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
            intervals.push([start, end]);
            start = end + 1;
        }

        // Send each interval to a different server
        // For the first server, we store the client socket
        let _first_task_id = if let Some(first_server) = servers.first() {
            let task_id = uuid::Uuid::new_v4().to_string();

            // Get the download interval for the first server
            let first_interval = if !intervals.is_empty() {
                intervals[0]
            } else {
                let file_size = file_info.file_size;
                let end = file_size.saturating_sub(1);
                [0, end]
            };

            // Store the client socket for the first task
            let mut tasks = pending_tasks.lock().await;
            tasks.insert(task_id.clone(), client_socket);

            // Create download task for the first server
            let download_task = DownloadTask {
                url: file_info.final_url.clone(),
                download_interval: first_interval,
                task_id: task_id.clone(),
            };

            // Send task to first server through long-lived connection
            if let Err(e) = Self::send_task_to_server_through_connection(first_server, download_task, server_connections.clone()).await {
                eprintln!("Failed to send task to server: {}", e);

                // Remove the pending task if we failed to send it
                let mut tasks = pending_tasks.lock().await;
                tasks.remove(&task_id);
            } else {
                println!("Sent download task {} to server {} with interval [{}, {}]",
                         task_id, first_server.id, first_interval[0], first_interval[1]);
            }

            Some(task_id)
        } else {
            None
        };

        // For remaining servers, send tasks without storing client socket
        for (i, server) in servers.iter().enumerate().skip(1) {
            if i >= intervals.len() {
                break; // No more intervals to send
            }

            let task_id = uuid::Uuid::new_v4().to_string();

            // Get the download interval for this server
            let interval = intervals[i];

            // Create download task
            let download_task = DownloadTask {
                url: download_request.url.clone(),
                download_interval: interval,
                task_id: task_id.clone(),
            };

            // Send task to server through long-lived connection
            if let Err(e) = Self::send_task_to_server_through_connection(server, download_task, server_connections.clone()).await {
                eprintln!("Failed to send task to server: {}", e);
            } else {
                println!("Sent download task {} to server:{} with interval [{}, {}]",
                         task_id, server.id, interval[0], interval[1]);
            }
        }

        Ok(())
    }

    /// Send a download task to a server
    async fn send_task_to_server(server_info: &ServerInfo, task: DownloadTask) -> Result<()> {
        let mut socket = TcpStream::connect(format!("{}", server_info.id)).await?;

        // Send task command
        socket.write_all(b"download_task").await?;

        // Wait for acknowledgment
        let mut buffer = [0; 2048];
        let n = socket.read(&mut buffer).await?;
        let response = String::from_utf8_lossy(&buffer[..n]);

        if response == "go_ahead" {
            // Send the task details
            let task_str = serde_json::to_string(&task)?;
            socket.write_all(task_str.as_bytes()).await?;
        }

        Ok(())
    }

    /// Get the current server list
    pub fn get_server_list(&self) -> Vec<ServerInfo> {
        let list = self.server_list.lock().unwrap();
        list.clone()
    }
}