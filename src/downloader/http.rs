//! HTTP downloader implementation

use reqwest::Client;
use tokio::io::AsyncWriteExt;
use indicatif::{ProgressBar, ProgressStyle};
use crate::error::Result;
use futures_util::StreamExt;

/// HTTP downloader
pub struct HttpDownloader {
    client: Client,
}

impl HttpDownloader {
    /// Create a new HTTP downloader
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .build()
            .expect("Failed to create HTTP client");
        Self { client }
    }

    /// Send a HEAD request to check if range requests are supported
    pub async fn partial_supported(&self, url: &str) -> Result<bool> {
        let response = self.client.head(url).send().await?;
        let partial_supported = response.headers().get("Accept-Ranges")
            .map(|val| val.to_str().unwrap_or("") == "bytes")
            .unwrap_or(false);
        Ok(partial_supported)
    }

    /// Download a file segment with range request
    pub async fn partial_request(
        &self,
        url: &str,
        left_point: u64,
        right_point: u64,
        file_path: &str,
    ) -> Result<()> {
        let range_header = format!("bytes={}-{}", left_point, right_point);

        let response = self.client
            .get(url)
            .header("Range", range_header)
            .send()
            .await?;

        let total_size = (right_point - left_point + 1) as u64;
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"));

        let mut file = tokio::fs::File::create(file_path).await?;
        let mut stream = response.bytes_stream();

        while let Some(item) = stream.next().await {
            let chunk = item?;
            file.write_all(&chunk).await?;
            pb.inc(chunk.len() as u64);
        }

        pb.finish_with_message("Download complete");
        Ok(())
    }

    /// Download a file segment (single-threaded)
    async fn single_thread_download(
        &self,
        url: &str,
        left_point: u64,
        right_point: u64,
        file_path: &str,
    ) -> Result<()> {
        println!("Downloading {}, [{} -> {}] ", url, left_point, right_point);
        self.partial_request(url, left_point, right_point, file_path).await
    }

    /// Download a file segment using single-threaded download (multi-threading disabled)
    pub async fn download_segment(
        &self,
        url: &str,
        left_point: u64,
        right_point: u64,
        _tmp_dir: &str,
        target_file_path: &str,
        _thread_number: usize,
    ) -> Result<()> {
        // Always use single-threaded download as multi-threading is disabled
        self.single_thread_download(
            url,
            left_point,
            right_point,
            target_file_path,
        ).await?;

        Ok(())
    }
}

impl Clone for HttpDownloader {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
        }
    }
}