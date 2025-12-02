//! Server network communication

use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde::{Deserialize, Serialize};
use crate::error::Result;
use crate::downloader::HttpDownloader;
// sleep is not currently used, but kept for potential future use
// use tokio::time::sleep;
// Duration is not currently used, but kept for potential future use
// use tokio::time::Duration;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

/// Metadata for file download
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadMetadata {
    pub url: String,
    pub download_interval: [u64; 2],
}

/// Task to be sent to server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub url: String,
    pub download_interval: [u64; 2],
    pub task_id: String,  // Unique identifier for this task
}

/// Task result to be sent back to manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Server network handler
pub struct ServerNetwork {
    http_downloader: HttpDownloader,
    tmp_dir: String,
    target_dir: String,
    thread_number: usize,
    manager_addr: String,
    manager_port: u16,
    id: u16,
    // Long-lived connection to manager
    manager_connection: Arc<TokioMutex<Option<TcpStream>>>,
}

impl ServerNetwork {
    /// Create a new server network handler
    pub fn new(
        id: u16,
        tmp_dir: String,
        target_dir: String,
        thread_number: usize,
    ) -> Self {
        let http_downloader = HttpDownloader::new();
        Self {
            id,
            http_downloader,
            tmp_dir,
            target_dir,
            thread_number,
            manager_addr: String::new(),
            manager_port: 0,
            manager_connection: Arc::new(TokioMutex::new(None)),
        }
    }

    /// Set manager connection info
    pub fn set_manager_info(&mut self, addr: String, port: u16) {
        self.manager_addr = addr;
        self.manager_port = port;
    }

    /// Set server connection info
    pub fn set_server_info(&mut self, id: u16) {
        self.id = id;
    }

    /// Establish and maintain long connection with manager
    pub async fn establish_long_connection(&self) -> Result<()> {
            match self.connect_to_manager().await {
                Ok(mut socket) => {
                    println!("Successfully established long connection with manager");

                    // Register with manager
                    if let Ok(()) = self.register_with_manager(&mut socket).await {
                        println!("Successfully registered with manager. Will now listen for tasks.");
                        // After successful registration, start listening for tasks on the same connection
                        self.listen_for_manager_tasks(socket).await
                    } else {
                        Err(crate::error::DistributedDownloaderError::NetworkError("Failed to register with manager".to_string()))
                    }
                }
                Err(e) => {
                    eprintln!("Failed to connect to manager: {}", e);
                    Err(e)
                }
            }
    }

    /// Connect to manager
    async fn connect_to_manager(&self) -> Result<TcpStream> {
        let socket = TcpStream::connect(format!("{}:{}", self.manager_addr, self.manager_port)).await?;
        println!("Connected to manager at {}:{}", self.manager_addr, self.manager_port);
        Ok(socket)
    }

    /// Register with manager
    async fn register_with_manager(&self, socket: &mut TcpStream) -> Result<()> {
        // Send registration message
        socket.write_all(b"server_register").await?;

        // Wait for acknowledgment
        let mut buffer = [0; 2048];
        let n = socket.read(&mut buffer).await?;
        let response = String::from_utf8_lossy(&buffer[..n]);

        if response == "go_ahead" {
            // Send server info
            let server_info = serde_json::json!({
                "id": self.id,
            });
            let server_info_str = serde_json::to_string(&server_info)?;
            socket.write_all(server_info_str.as_bytes()).await?;

            // Wait for final acknowledgment
            let mut buffer = [0; 2048];
            let n = socket.read(&mut buffer).await?;
            let response = String::from_utf8_lossy(&buffer[..n]);

            if response == "registered" {
                println!("Successfully registered with manager");
                Ok(())
            } else {
                Err(crate::error::DistributedDownloaderError::NetworkError("Registration failed".to_string()))
            }
        } else {
            Err(crate::error::DistributedDownloaderError::NetworkError("Registration not acknowledged".to_string()))
        }
    }

    /// Listen for tasks from manager
    async fn listen_for_manager_tasks(&self, mut socket: TcpStream) -> Result<()> {
        println!("Listening for tasks from manager...");

        loop {
            let mut buffer = [0; 8192];
            let n = socket.read(&mut buffer).await?;

            if n == 0 {
                println!("Connection to manager closed");
                break;
            }

            let message = String::from_utf8_lossy(&buffer[..n]);
            println!("Received message from manager: {}", message);

            // Check if this is a download task
            if message == "download_task" {
                // Acknowledge receipt
                socket.write_all(b"go_ahead").await?;

                // Receive task details
                let mut buffer = [0; 8192];
                let n = socket.read(&mut buffer).await?;
                let task_str = String::from_utf8_lossy(&buffer[..n]);
                let task: DownloadTask = serde_json::from_str(&task_str)?;

                println!("Received download task: {:?}", task);

                // Process the download task in a separate task
                let server_clone = self.clone();
                let manager_addr = self.manager_addr.clone();
                let manager_port = self.manager_port;
                tokio::spawn(async move {
                    // Create a new connection to send the response
                    // We need a new connection because the main connection is still listening for tasks
                    if let Ok(response_socket) = TcpStream::connect(format!("{}:{}", manager_addr, manager_port)).await {
                        if let Err(e) = server_clone.process_download_task(response_socket, task).await {
                            eprintln!("Error processing download task: {}", e);
                        }
                    } else {
                        eprintln!("Failed to connect to manager for sending response");
                    }
                });
            } else {
                eprintln!("Unknown message from manager: {}", message);
            }
        }

        Ok(())
    }

    /// Process a download task from manager
    async fn process_download_task(&self, mut socket: TcpStream, task: DownloadTask) -> Result<()> {
        println!("Processing download task: {}", task.task_id);

        // Download file segment
        let file_name = uuid::Uuid::new_v4().to_string();
        let file_path = format!("{}/{}", self.target_dir, file_name);

        let result = self.http_downloader
            .download_segment(
                &task.url,
                task.download_interval[0],
                task.download_interval[1],
                &self.tmp_dir,
                &file_path,
                self.thread_number,
            )
            .await;

        // Send task completion message
        let _task_result = if result.is_ok() {
            // Send file segment back to manager
            socket.write_all(format!("task_complete:{}", task.task_id).as_bytes()).await?;

            let mut file = tokio::fs::File::open(&file_path).await?;
            let mut buffer = [0; 2048];

            loop {
                let n = file.read(&mut buffer).await?;
                if n == 0 {
                    break;
                }
                socket.write_all(&buffer[..n]).await?;
            }

            // Clean up downloaded file
            tokio::fs::remove_file(&file_path).await?;

            TaskResult {
                task_id: task.task_id.clone(),
                success: true,
                error_message: None,
            }
        } else {
            let error_msg = format!("Download failed: {}", result.err().unwrap());
            eprintln!("{}", error_msg);

            TaskResult {
                task_id: task.task_id.clone(),
                success: false,
                error_message: Some(error_msg),
            }
        };

        println!("Completed download task: {}", task.task_id);
        Ok(())
    }

    /// Send a heartbeat message to manager to check connection health
    pub async fn send_heartbeat(&self) -> Result<()> {
        // For now, we'll just return Ok
        // In a more advanced implementation, we could send a heartbeat message
        // and wait for a response to verify the connection is still alive
        Ok(())
    }

    /// Notify manager that server is up (deprecated - using long connection now)
    pub async fn notify_manager_up(&self, _manager_addr: &str, _manager_port: u16, _server_addr: &str, _server_port: u16) -> Result<()> {
        // This is deprecated in the new architecture
        Ok(())
    }

    /// Notify manager that server is down (deprecated - using long connection now)
    pub async fn notify_manager_down(&self, _manager_addr: &str, _manager_port: u16, _server_addr: &str, _server_port: u16) -> Result<()> {
        // This is deprecated in the new architecture
        Ok(())
    }

    /// Listen for tasks from manager using the established connection
    pub async fn listen_for_tasks(&self) -> Result<()> {
        // For now, we'll create a new connection as TcpStream cannot be shared directly
        // In a more advanced implementation, we could use channels or other IPC mechanisms
        let socket = TcpStream::connect(format!("{}:{}", self.manager_addr, self.manager_port)).await?;
        self.listen_for_manager_tasks(socket).await
    }
}

impl Clone for ServerNetwork {
    fn clone(&self) -> Self {
        Self {
            http_downloader: self.http_downloader.clone(),
            tmp_dir: self.tmp_dir.clone(),
            target_dir: self.target_dir.clone(),
            thread_number: self.thread_number,
            manager_addr: self.manager_addr.clone(),
            manager_port: self.manager_port,
            id: self.id,
            manager_connection: self.manager_connection.clone(),
        }
    }
}