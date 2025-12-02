//! HTTP downloader implementation

use reqwest::Client;
use tokio::io::AsyncWriteExt;
use indicatif::{ProgressBar, ProgressStyle};
use crate::error::Result;
use crate::utils::distributor::DownloadDistributor;
use futures_util::StreamExt;

/// HTTP downloader
pub struct HttpDownloader {
    client: Client,
}

impl HttpDownloader {
    /// Create a new HTTP downloader
    pub fn new() -> Self {
        let client = Client::new();
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
        println!("Multi-threaded downloading is not supported by {}. Downloading will be single-threaded.", url);
        self.partial_request(url, left_point, right_point, file_path).await
    }

    /// Download a file segment (multi-threaded)
    async fn multi_thread_download(
        &self,
        url: &str,
        left_point: u64,
        right_point: u64,
        tmp_dir: &str,
        thread_number: usize,
    ) -> Result<Vec<String>> {
        println!("Trying multi-threaded download [{}, {}] from: {}", left_point, right_point, url);

        let download_intervals = DownloadDistributor::download_interval_list(
            left_point,
            right_point,
            thread_number,
        );

        let mut handles = vec![];
        let mut file_paths = vec![];

        for (thread_id, interval) in download_intervals.into_iter().enumerate() {
            let file_path = format!("{}/segment_by_thread_{}", tmp_dir, thread_id);
            file_paths.push(file_path.clone());

            let downloader = self.clone();
            let url = url.to_string();

            let handle = tokio::spawn(async move {
                downloader.partial_request(
                    &url,
                    interval[0],
                    interval[1],
                    &file_path,
                ).await
            });

            handles.push(handle);
        }

        // Wait for all downloads to complete
        for handle in handles {
            handle.await??;
        }

        Ok(file_paths)
    }

    /// Download a file segment, automatically choosing between single-threaded and multi-threaded
    pub async fn download_segment(
        &self,
        url: &str,
        left_point: u64,
        right_point: u64,
        tmp_dir: &str,
        target_file_path: &str,
        thread_number: usize,
    ) -> Result<()> {
        if self.partial_supported(url).await? {
            // Multi-threaded download
            let file_segments = self.multi_thread_download(
                url,
                left_point,
                right_point,
                tmp_dir,
                thread_number,
            ).await?;

            // Merge file segments
            crate::utils::append_files(&file_segments, target_file_path)?;

            // Clean up file segments
            for segment in file_segments {
                crate::utils::delete_file(&segment)?;
            }
        } else {
            // Single-threaded download
            self.single_thread_download(
                url,
                left_point,
                right_point,
                target_file_path,
            ).await?;
        }

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