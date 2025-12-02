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

/// Information about a download task for tracking purposes
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub download_interval: [u64; 2],
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
                            task_infos,
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
                        // First, get the task info to get the download interval
                        let task_info = {
                            let mut infos = task_infos.lock().await;
                            infos.remove(&task_id) // Remove and get the task info
                        };

                        if let Some(task_info) = task_info {
                            let mut tasks = pending_tasks.lock().await;
                            if let Some(mut client_socket) = tasks.remove(&task_id) {
                                // Send the task ID and download interval
                                let interval_msg = format!("task_data:{}:{}-{}", task_id, task_info.download_interval[0], task_info.download_interval[1]);
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
                            eprintln!("Task info not found for task: {}", task_id);
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
            intervals.push([start, end]);
            start = end + 1;
        }

        // Send each interval to a different server
        // For all servers, we need to store the client socket so we can forward data back to the client
        // We'll create a collection to hold all the task IDs that should forward to the same client
        let mut task_ids_to_client = Vec::new();

        for (i, server) in servers.iter().enumerate() {
            if i >= intervals.len() {
                break; // No more intervals to send
            }

            let task_id = uuid::Uuid::new_v4().to_string();
            task_ids_to_client.push(task_id.clone());

            // Get the download interval for this server
            let interval = intervals[i];

            // Create download task
            let download_task = DownloadTask {
                url: file_info.final_url.clone(),
                download_interval: interval,
                task_id: task_id.clone(),
            };

            // Store task info for tracking download intervals
            let task_info = TaskInfo {
                download_interval: interval,
            };
            let mut infos = task_infos.lock().await;
            infos.insert(task_id.clone(), task_info);

            // Send task to server through long-lived connection
            if let Err(e) = Self::send_task_to_server_through_connection(server, download_task, server_connections.clone()).await {
                eprintln!("Failed to send task to server: {}", e);
                // Remove the task info if we failed to send the task
                let mut infos = task_infos.lock().await;
                infos.remove(&task_id);
            } else {
                println!("Sent download task {} to server:{} with interval [{}, {}]",
                         task_id, server.id, interval[0], interval[1]);
            }
        }

        // Store the client socket for all tasks (this is a simplified approach)
        // In a real implementation, we would need a more sophisticated way to handle this
        if !task_ids_to_client.is_empty() {
            let first_task_id = &task_ids_to_client[0];
            let mut tasks = pending_tasks.lock().await;
            tasks.insert(first_task_id.clone(), client_socket);
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