
use anyhow::Result;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use tonic::Request;
use tracing::{info, warn};
use url::Url;

use crate::proto::distributed_downloader::{
    client_data_chunk, manager_service_client::ManagerServiceClient, DownloadRequest,
};

/// Command-line arguments for the client.
#[derive(clap::Args, Debug)]
pub struct Args {
    /// The URL of the file to download.
    #[clap()]
    pub url: String,

    /// The output file name. If not provided, the filename will be extracted from the URL.
    #[clap(short, long)]
    pub output: Option<String>,

    /// Address of the manager (e.g., "http://127.0.0.1:5000").
    #[clap(short, long, default_value = "http://127.0.0.1:5000")]
    pub manager_address: String,
}

/// Runs the client.
pub async fn run(args: &Args) -> Result<()> {
    // Note: Tracing is initialized in the main binary.
    info!("Connecting to manager at {}...", args.manager_address);
    let mut client = ManagerServiceClient::connect(args.manager_address.clone()).await?;
    info!("Successfully connected to manager.");

    let output_file_name = if let Some(output) = &args.output {
        PathBuf::from(output)
    } else {
        let parsed_url = Url::parse(&args.url)?;
        let segments = parsed_url
            .path_segments()
            .ok_or_else(|| anyhow::anyhow!("Could not get path segments from URL"))?;
        let filename_str = segments
            .last()
            .ok_or_else(|| anyhow::anyhow!("Could not get last path segment from URL"))?;

        if filename_str.is_empty() {
            warn!("Could not extract filename from URL, using 'downloaded_file' as default.");
            PathBuf::from("downloaded_file")
        } else {
            PathBuf::from(filename_str)
        }
    };

    let request = Request::new(DownloadRequest {
        url: args.url.clone(),
        output_file: output_file_name.to_string_lossy().into_owned(),
    });

    info!("Sending download request to manager...");
    let mut stream = client.request_download(request).await?.into_inner();

    // 1. Wait for the first message (metadata) to set up the file and progress bar.
    let (mut file, pb) = if let Some(first_msg_result) = stream.next().await {
        let first_msg = first_msg_result?;
        if let Some(client_data_chunk::Payload::Metadata(metadata)) = first_msg.payload {
            info!(
                "Received metadata for job '{}'. File size: {} bytes.",
                metadata.job_id, metadata.file_size
            );

            let pb = ProgressBar::new(metadata.file_size);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")?
                    .progress_chars("#>-"),
            );

            // Create and pre-allocate the output file
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .open(&output_file_name)?;
            file.set_len(metadata.file_size)?;

            Ok((file, pb))
        } else {
            Err(anyhow::anyhow!("First message from server was not metadata."))
        }
    } else {
        Err(anyhow::anyhow!(
            "Manager closed stream before sending metadata."
        ))
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
    info!(
        "File '{}' has been downloaded successfully.",
        output_file_name.to_string_lossy()
    );

    Ok(())
}
