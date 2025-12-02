//! Client network communication

use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncSeekExt};
use crate::error::{DistributedDownloaderError, Result};
use crate::network::manager::{ServerInfo, DownloadRequest, FileInfoResponse};
use tokio::fs::File;
use std::collections::HashMap;

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
                let file = File::create(file_path).await?;
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

                // Create a temporary file to store the downloaded data
                let temp_file_path = format!("{}.tmp", file_path);
                let temp_file = File::create(&temp_file_path).await?;

                // Create a map to track the expected data ranges
                let mut expected_ranges: HashMap<String, (u64, u64)> = HashMap::new();

                // Reopen the file for random access writing
                drop(temp_file); // Close the temporary file
                let mut file = File::create(file_path).await?;
                file.set_len(file_info.file_size).await?; // Pre-allocate disk space

                // Track the current task context for data writing
                let mut current_task_id: Option<String> = None;

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
                        // This is a task data marker with format "task_data:{task_id}:{start}-{end}"
                        // Parse the start and end positions
                        let parts: Vec<&str> = message.split(':').collect();
                        if parts.len() == 3 {
                            let range_parts: Vec<&str> = parts[2].split('-').collect();
                            if range_parts.len() == 2 {
                                if let (Ok(start_pos), Ok(end_pos)) = (range_parts[0].parse::<u64>(), range_parts[1].parse::<u64>()) {
                                    println!("Received task data for range {}-{}", start_pos, end_pos);

                                    // Store the range info for this task
                                    let task_id = parts[1].to_string();
                                    expected_ranges.insert(task_id.clone(), (start_pos, end_pos));
                                    current_task_id = Some(task_id);
                                    continue; // Continue to read the actual data
                                }
                            }
                        }
                        // If we can't parse the range info, just continue reading
                        current_task_id = None;
                        continue;
                    } else {
                        // This is file data, write it to the file at the correct position
                        if let Some(ref task_id) = current_task_id {
                            if let Some(&(start_pos, _end_pos)) = expected_ranges.get(task_id) {
                                // Seek to the correct position and write the data
                                file.seek(tokio::io::SeekFrom::Start(start_pos)).await?;
                                file.write_all(data).await?;

                                // Update the range info to reflect that we've written this data
                                // This is a simplified approach - in a real implementation you might track progress more precisely
                                println!("Wrote {} bytes at position {} for task {}", data.len(), start_pos, task_id);
                            }
                        } else {
                            // If we don't have task context, just append (fallback behavior)
                            file.write_all(data).await?;
                        }
                    }
                }

                // Clean up the temporary file if it still exists
                let temp_file_path = format!("{}.tmp", file_path);
                let _ = tokio::fs::remove_file(&temp_file_path).await;
            }
        }

        Ok(())
    }

}