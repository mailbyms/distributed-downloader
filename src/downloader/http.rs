//! HTTP downloader implementation

use crate::error::Result;
use bytes::Bytes;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;

/// A new, standalone function to download a specific byte range from a URL into memory.
pub async fn download_range(url: &str, start: u64, end: u64) -> Result<Bytes> {
    let downloader = HttpDownloader::new();
    downloader.partial_request(url, start, end).await
}

/// HTTP Downloader
pub struct HttpDownloader {
    client: Client,
}

impl HttpDownloader {
    /// Creates a new HTTP downloader
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .build()
            .expect("Failed to build HTTP client");
        Self { client }
    }

    /// Downloads a segment using a range request into a Bytes buffer, with a progress bar.
    pub async fn partial_request(&self, url: &str, left_point: u64, right_point: u64) -> Result<Bytes> {
        let range_header = format!("bytes={}-{}", left_point, right_point);

        let response = self
            .client
            .get(url)
            .header("Range", range_header)
            .send()
            .await?;
        
        let status = response.status();
        if !status.is_success() {
            return Err(crate::error::DistributedDownloaderError::HttpError(
                format!("HTTP request failed with status: {}", status)
            ));
        }

        let total_size = right_point.saturating_sub(left_point) + 1;
        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .unwrap()
                .progress_chars("#>-"),
        );

        let mut stream = response.bytes_stream();
        let mut downloaded_data = Vec::with_capacity(total_size as usize);

        while let Some(item) = stream.next().await {
            let chunk = item?;
            downloaded_data.extend_from_slice(&chunk);
            pb.inc(chunk.len() as u64);
        }

        pb.finish_with_message("Chunk download complete");
        Ok(Bytes::from(downloaded_data))
    }
}

impl Clone for HttpDownloader {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
        }
    }
}