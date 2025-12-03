//! Client binary for the Distributed Downloader.

use anyhow::Result;
use clap::Parser;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use tonic::Request;
use tracing::{info, warn};

use distributed_downloader::{
    // config::ClientConfig, // Removed
    proto::distributed_downloader::{
        client_data_chunk, manager_service_client::ManagerServiceClient, DownloadRequest,
    },
};

/// Command-line arguments for the client.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The URL of the file to download.
    #[clap()]
    url: String,

    /// The output file name.
    #[clap(short, long)]
    output: String,

    /// Address of the manager (e.g., "http://127.0.0.1:5000").
    #[clap(short, long, default_value = "http://127.0.0.1:5000")]
    manager_address: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    info!("Connecting to manager at {}...", args.manager_address);
    let mut client = ManagerServiceClient::connect(args.manager_address).await?;
    info!("Successfully connected to manager.");

    let request = Request::new(DownloadRequest {
        url: args.url.clone(),
        output_file: args.output.clone(),
    });

    info!("Sending download request to manager...");
    let mut stream = client.request_download(request).await?.into_inner();

    // 1. Wait for the first message (metadata) to set up the file and progress bar.
    let (mut file, pb) = if let Some(first_msg_result) = stream.next().await {
        let first_msg = first_msg_result?;
        if let Some(client_data_chunk::Payload::Metadata(metadata)) = first_msg.payload {
            info!("Received metadata for job '{}'. File size: {} bytes.", metadata.job_id, metadata.file_size);

            let pb = ProgressBar::new(metadata.file_size);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")?
                    .progress_chars("#>-"),
            );

            // Create and pre-allocate the output file
            let file = OpenOptions::new().write(true).create(true).open(&args.output)?;
            file.set_len(metadata.file_size)?;
            
            Ok((file, pb))
        } else {
            Err(anyhow::anyhow!("First message from server was not metadata."))
        }
    } else {
        Err(anyhow::anyhow!("Manager closed stream before sending metadata."))
    }?;

    // 2. Process subsequent data chunks.
    while let Some(msg_result) = stream.next().await {
        let msg = msg_result?;
        if let Some(client_data_chunk::Payload::Chunk(chunk)) = msg.payload {
            file.seek(SeekFrom::Start(chunk.offset))?;
            file.write_all(&chunk.data)?;
            pb.inc(chunk.data.len() as u64);
        } else {
            // This case should ideally not be reached if the server adheres to the protocol.
            warn!("Received a non-chunk message after metadata.");
        }
    }

    pb.finish_with_message("Download complete");
    info!("File '{}' has been downloaded successfully.", args.output);

    Ok(())
}