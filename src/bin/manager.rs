//! Main binary for the Manager node.

use anyhow::Result;
use clap::Parser;
use std::time::Duration;
use tonic::transport::Server;
use tracing::{info, warn};

use distributed_downloader::{
    config::ManagerConfig,
    network::manager::{ManagerServiceImpl, ServerStatus},
    proto::distributed_downloader::{
        manager_command::Payload as ManagerPayload, manager_service_server::ManagerServiceServer,
        ManagerCommand,
    },
};

/// Command-line arguments for the manager.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the manager configuration file.
    #[clap(short, long, default_value = "configs/manager.yml")]
    config: String,
}

/// The core scheduler loop.
async fn run_scheduler(service: ManagerServiceImpl) {
    info!("Scheduler started.");
    loop {
        let mut job_id_to_clean = None;

        // --- Scope for checking the task queue ---
        {
            let mut task_queue = service.task_queue.lock().await;

            // 1. Peek at the next task to see if it needs cleanup
            if let Some(task) = task_queue.front() {
                let job_id = &task.job_id;
                let mut cleanup = false;

                if let Some(job_entry) = service.jobs.jobs.get(job_id) {
                    let job = job_entry.lock().await;
                    if job.sender.is_closed() {
                        // Client is gone, schedule cleanup for this job
                        cleanup = true;
                    }
                } else {
                    // Job is no longer tracked (e.g., completed or already cancelled)
                    cleanup = true;
                }

                if cleanup {
                    job_id_to_clean = Some(job_id.clone());
                }
            }

            // 2. If cleanup is needed, do it now while holding the queue lock
            if let Some(job_id) = job_id_to_clean {
                warn!("[Scheduler] Client for job {} disconnected. Cleaning up.", job_id);
                service.jobs.jobs.remove(&job_id);
                task_queue.retain(|t| t.job_id != job_id);
                info!("[Scheduler] Purged all tasks for job {}.", job_id);
                // Continue to the next loop iteration to re-evaluate state
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
        } // All locks on task_queue are dropped here

        // 3. Find an idle server
        let mut idle_server = None;
        for entry in service.servers.servers.iter() {
            let mut status = entry.value().status.lock().await;
            if *status == ServerStatus::Idle {
                // Tentatively mark as busy to avoid race conditions
                *status = ServerStatus::Busy("pending_assignment".to_string());
                idle_server = Some((entry.key().clone(), entry.value().clone()));
                break;
            }
        }

        // 4. If we have both an idle server and a task, dispatch it
        if let Some((server_id, server_conn)) = idle_server {
            let mut task_queue = service.task_queue.lock().await;
            if let Some(task) = task_queue.pop_front() {
                // We have a task and a server, so let's dispatch
                info!("[Scheduler] Assigning task {} to server {}", task.task_id, server_id);
                let command = ManagerCommand {
                    payload: Some(ManagerPayload::AssignTask(task.clone())),
                };

                // Send the task
                if server_conn.sender.send(Ok(command)).await.is_ok() {
                    // Finalize assignment by updating status with the correct task_id
                    *server_conn.status.lock().await = ServerStatus::Busy(task.task_id);
                } else {
                    // Sending failed, server likely disconnected.
                    warn!("[Scheduler] Failed to send task to server {}. Removing server and re-queueing task.", server_id);
                    service.servers.servers.remove(&server_id);
                    // Re-queue the task at the front
                    task_queue.push_front(task);
                }
            } else {
                // No task was available, so set the server back to idle
                *server_conn.status.lock().await = ServerStatus::Idle;
            }
        }

        // Wait a bit before the next cycle
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}


#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    info!("Loading manager configuration from: {}", args.config);
    let config = ManagerConfig::from_file(&args.config)?;
    let addr = format!("{}:{}", config.manager_addr_ipv4, config.manager_port).parse()?;

    let manager_service = ManagerServiceImpl::new();

    // Spawn the scheduler as a background task
    tokio::spawn(run_scheduler(manager_service.clone()));
    
    let svc = ManagerServiceServer::new(manager_service)
        .max_decoding_message_size(16 * 1024 * 1024); // 16 MB

    info!("Manager gRPC service listening on {}", addr);

    // Build and start the tonic gRPC server
    Server::builder().add_service(svc).serve(addr).await?;

    Ok(())
}