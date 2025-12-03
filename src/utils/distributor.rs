//! Contains the logic for distributing download tasks to available servers.

use anyhow::{anyhow, Result};
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::{
    network::manager::ConnectionManager,
    proto::distributed_downloader::{
        manager_command::Payload as ManagerPayload, DownloadTask, ManagerCommand,
    },
};

const MAX_CHUNK_SIZE: u64 = 3 * 1024 * 1024; // 3 MB

/// The Distributor is responsible for splitting a download job into tasks
/// and dispatching them to available servers.
#[derive(Debug)]
pub struct Distributor {
    connections: ConnectionManager,
}

impl Distributor {
    /// Creates a new Distributor.
    pub fn new(connections: ConnectionManager) -> Self {
        Self { connections }
    }

    /// Generates a list of byte range tasks for a given file size,
    /// ensuring no task is larger than MAX_CHUNK_SIZE.
    fn generate_tasks(file_size: u64) -> Vec<(u64, u64)> {
        let mut tasks = Vec::new();
        let mut current_pos = 0;

        while current_pos < file_size {
            let end_pos = (current_pos + MAX_CHUNK_SIZE - 1).min(file_size - 1);
            tasks.push((current_pos, end_pos));
            current_pos = end_pos + 1;
        }
        tasks
    }

    /// Takes a download job, splits it into tasks, and sends them to connected servers.
    pub async fn distribute_and_dispatch(
        &self,
        job_id: String,
        url: String,
        file_size: u64,
    ) -> Result<()> {
        let servers = self.connections.get_all_servers();
        let num_servers = servers.len();

        if num_servers == 0 {
            warn!("No servers available to dispatch tasks for job {}.", job_id);
            return Err(anyhow!("No servers available for download."));
        }

        let tasks = Self::generate_tasks(file_size);
        info!(
            "Distributing job {} ({} bytes) into {} tasks among {} servers.",
            job_id, file_size, tasks.len(), num_servers
        );
        
        // Distribute tasks to servers in a round-robin fashion.
        for (i, (range_start, range_end)) in tasks.iter().enumerate() {
            let (server_id, connection) = &servers[i % num_servers];
            
            let task_id = Uuid::new_v4().to_string();
            let task = DownloadTask {
                job_id: job_id.clone(),
                task_id: task_id.clone(),
                url: url.clone(),
                range_start: *range_start,
                range_end: *range_end,
            };

            let command = ManagerCommand {
                payload: Some(ManagerPayload::AssignTask(task)),
            };
            
            info!("Assigning task {} ({}..{}) to server {}", task_id, range_start, range_end, server_id);

            // Send the command to the server through its channel.
            if let Err(e) = connection.sender.send(Ok(command)).await {
                error!("Failed to send task to server {}: {}. The server might have disconnected.", server_id, e);
                // In a real-world scenario, you might want to re-queue this task.
            }
        }

        Ok(())
    }
}