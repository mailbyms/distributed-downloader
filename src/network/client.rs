//! Client network communication

use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::error::{DistributedDownloaderError, Result};
use crate::network::manager::{ServerInfo, DownloadRequest, FileInfoResponse};
use tokio::fs::File;

/// Client network handler
pub struct ClientNetwork;

impl ClientNetwork {
    /// Request server list from manager
    pub async fn ask_manager_for_server_list(
        manager_addr: &str,
        manager_port: u16,
    ) -> Result<Vec<ServerInfo>> {
        let mut socket = TcpStream::connect(format!("{}:{}", manager_addr, manager_port)).await?;

        socket.write_all(b"ask_for_server_list").await?;

        let mut buffer = vec![0; 4096];
        let n = socket.read(&mut buffer).await?;
        let server_list_str = String::from_utf8_lossy(&buffer[..n]);

        let server_list: Vec<ServerInfo> = serde_json::from_str(&server_list_str)?;

        Ok(server_list)
    }

    /// Send download request to manager and receive file segments
    pub async fn request_download_via_manager(
        manager_addr: &str,
        manager_port: u16,
        url: &str,
        file_path: &str,
    ) -> Result<()> {
        let mut socket = TcpStream::connect(format!("{}:{}", manager_addr, manager_port)).await?;

        // Send download request command
        socket.write_all(b"download_request").await?;

        // Wait for acknowledgment
        let mut buffer = [0; 2048];
        let n = socket.read(&mut buffer).await?;
        let response = String::from_utf8_lossy(&buffer[..n]);

        if response == "go_ahead" {
            // Send download request
            let download_request = DownloadRequest {
                url: url.to_string(),
            };
            let request_str = serde_json::to_string(&download_request)?;
            socket.write_all(request_str.as_bytes()).await?;

            // Read file info response
            let mut buffer = [0; 8192];
            let n = socket.read(&mut buffer).await?;
            let response_str = String::from_utf8_lossy(&buffer[..n]);

            if response_str.starts_with("file_info:") {
                let file_info_str = &response_str[10..]; // Skip "file_info:" prefix
                let file_info: FileInfoResponse = serde_json::from_str(file_info_str)?;

                println!("Received file info: size={}, chunks={}", file_info.file_size, file_info.chunk_count);

                // Pre-allocate disk space for the file
                let mut file = File::create(file_path).await?;
                file.set_len(file_info.file_size).await?;

                // Send acknowledgment to manager
                socket.write_all(b"ready_for_download").await?;

                // Track received chunks and their positions
                let mut chunk_positions = vec![0u64; file_info.chunk_count];
                let mut pos = 0u64;
                for (i, &size) in file_info.chunk_sizes.iter().enumerate() {
                    chunk_positions[i] = pos;
                    pos += size;
                }

                // Receive file segments from manager
                loop {
                    let n = match socket.read(&mut buffer).await {
                        Ok(0) => break, // Connection closed
                        Ok(n) => n,
                        Err(e) => return Err(DistributedDownloaderError::NetworkError(e.to_string())),
                    };

                    // Check if this is a task data marker
                    let data = &buffer[..n];
                    let message = String::from_utf8_lossy(data);

                    if message.starts_with("task_data:") {
                        // This is a task data marker, the actual data follows
                        // In a real implementation, we would parse the task ID and handle accordingly
                        // For now, we'll just continue reading the data
                        continue;
                    } else {
                        // This is file data, write it to the file at the correct position
                        // For simplicity, we're just appending in this example
                        // In a real implementation, you would track which chunk this is and write to the correct position
                        file.write_all(data).await?;
                    }
                }
            }
        }

        Ok(())
    }

}